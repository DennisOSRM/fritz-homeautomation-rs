use clap::{App, Arg, ArgMatches};
use fritz_homeautomation::{api, daylight};
use std::process::exit;

fn valid_coord(val: String) -> Result<(), String> {
    val.parse::<f64>()
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn valid_date(val: String) -> Result<(), String> {
    chrono::NaiveDate::parse_from_str(&val, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn valid_shift(arg: String) -> Result<(), String> {
    parse_duration(&arg)
        .map(|_| ())
        .ok_or("Not a valid time shift".to_string())
}

fn parse_duration(arg: &str) -> Option<chrono::Duration> {
    let sign = arg.starts_with("-");
    let input = if sign { &arg[1..] } else { arg };
    match parse_duration::parse(input) {
        Err(err) => {
            eprintln!("{:?}", err);
            None
        }
        Ok(parsed) => chrono::Duration::from_std(parsed)
            .ok()
            .map(|val| if sign { -val } else { val }),
    }
}

fn daylight(args: &ArgMatches) {
    // get date arguments
    let date = args
        .value_of("date")
        .and_then(|val| chrono::NaiveDate::parse_from_str(val, "%Y-%m-%d").ok());
    let from_date = args
        .value_of("from-date")
        .and_then(|val| chrono::NaiveDate::parse_from_str(val, "%Y-%m-%d").ok());
    let to_date = args
        .value_of("to-date")
        .and_then(|val| chrono::NaiveDate::parse_from_str(val, "%Y-%m-%d").ok());
    let (from_date, to_date) = match (from_date, to_date, date) {
        (Some(from_date), Some(to_date), _) => (from_date, to_date),
        (_, _, Some(date)) => (date, date),
        _ => {
            let date = chrono::Local::today().naive_local();
            (date, date)
        }
    };

    // get shift
    let shift_from = args.value_of("shift-from").and_then(parse_duration);
    let shift_to = args.value_of("shift-to").and_then(parse_duration);

    // get location
    let latitude: Option<f64> = args.value_of("latitude").and_then(|val| val.parse().ok());
    let longitude: Option<f64> = args.value_of("longitude").and_then(|val| val.parse().ok());
    let location = match (latitude, longitude) {
        (Some(latitude), Some(longitude)) => daylight::Location::new(latitude, longitude),
        _ => {
            if let Ok(loc) = daylight::default_location() {
                loc
            } else {
                println!("Could not determine location for daylight time. Maybe use --latitude / --longitude?");
                exit(1);
            }
        }
    };

    daylight::print_daylight_times(location, from_date, to_date, shift_from, shift_to);
}

fn list(args: &ArgMatches) -> anyhow::Result<()> {
    let user = args.value_of("user").unwrap();
    let password = args.value_of("password").unwrap();
    let ain = args.value_of("ain");
    let show_stats = args.is_present("stats");

    let sid = api::get_sid(&user, &password)?;
    let devices: Vec<_> = api::device_infos_avm(&sid)?;

    if let Some(ain) = ain {
        let device = match devices.into_iter().find(|dev| dev.id() == ain) {
            None => {
                return Err(anyhow::anyhow!("Cannot find device with ain {:?}", ain));
            }
            Some(device) => device,
        };
        device.print_info(show_stats, Some(&sid))?;
        return Ok(());
    }

    println!("found {} devices", devices.len());

    for device in devices {
        device.print_info(show_stats, Some(&sid))?;
    }

    Ok(())
}

fn switch(args: &ArgMatches) -> anyhow::Result<()> {
    let user = args.value_of("user").unwrap();
    let password = args.value_of("password").unwrap();
    let ain = args.value_of("ain").unwrap();
    let toggle = args.is_present("toggle");
    let on = args.is_present("on");
    let off = args.is_present("off");

    let sid = api::get_sid(&user, &password)?;
    let devices: Vec<_> = api::device_infos_avm(&sid)?;

    let mut device = match devices.into_iter().find(|dev| dev.id() == ain) {
        None => {
            return Err(anyhow::anyhow!("Cannot find device with ain {:?}", ain));
        }
        Some(device) => device,
    };

    if toggle {
        device.toggle(&sid)?;
    } else if on {
        device.turn_on(&sid)?;
    } else if off {
        device.turn_off(&sid)?;
    }

    Ok(())
}

// -=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-

fn main() {
    let user = Arg::with_name("user")
        .long("user")
        .short("u")
        .takes_value(true)
        .required(true);

    let password = Arg::with_name("password")
        .long("password")
        .short("p")
        .takes_value(true)
        .required(true);

    let ain = Arg::with_name("ain")
        .long("ain")
        .takes_value(true)
        .required(true)
        .help("The device identifier of the device to query / control.");

    let mut app = App::new(env!("CARGO_PKG_NAME"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .subcommand(
            App::new("list")
                .arg(user.clone())
                .arg(password.clone())
                .arg(ain.clone().required(false))
                .arg(Arg::with_name("stats").long("stats")),
        )
        .subcommand(
            App::new("switch")
                .arg(user)
                .arg(password)
                .arg(ain.clone().required(true))
                .arg(Arg::with_name("toggle").long("toggle"))
                .arg(Arg::with_name("on").long("on"))
                .arg(Arg::with_name("off").long("off")),
        )
        .subcommand(
            App::new("daylight")
                .help("Prints the daylight times at a specific location. On MacOS will try to use the corelocation API if no latitude/longitude is specified.")
                .arg(Arg::with_name("latitude")
                     .long("latitude")
                     .takes_value(true)
                     .validator(valid_coord))
                .arg(Arg::with_name("longitude")
                     .long("longitude")
                     .takes_value(true)
                     .validator(valid_coord))
                .arg(Arg::with_name("date")
                     .long("date")
                     .takes_value(true)
                     .validator(valid_date))
                .arg(Arg::with_name("from-date")
                     .long("from-date")
                     .takes_value(true)
                     .validator(valid_date))
                .arg(Arg::with_name("to-date")
                     .long("to-date")
                     .takes_value(true)
                     .validator(valid_date))
                .arg(Arg::with_name("shift-from")
                     .long("shift-from")
                     .takes_value(true)
                     .validator(valid_shift))
                .arg(Arg::with_name("shift-to")
                     .long("shift-to")
                     .takes_value(true)
                     .validator(valid_shift))
        );

    let args = app.clone().get_matches();

    let cmd = match args.subcommand {
        None => {
            app.print_help().unwrap();
            exit(1);
        }
        Some(ref cmd) => cmd.name.as_str(),
    };

    match cmd {
        "daylight" => {
            let args = args.subcommand_matches("daylight").unwrap();
            daylight(args);
        }
        "list" => {
            list(args.subcommand_matches("list").unwrap()).unwrap();
        }
        "switch" => {
            switch(args.subcommand_matches("switch").unwrap()).unwrap();
        }
        _ => {
            app.print_help().unwrap();
            exit(1);
        }
    }
}