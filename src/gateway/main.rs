use heladeria::gateway::gateway::Gateway;
use heladeria::gateway::gateway::LogFile;
use std::env;
use std::fs;

const LOCAL_IP: &str = "127.0.0.1";
const GATEWAY_PORT: &u16 = &6000;
const LOG_FILE_PATH: &str = "transactions.log";
const DEFAULT_REJECTION_PERCENTAGE: u8 = 10;

/// Punto de entrada principal para el binario `gateway`.
/// Acepta un argumento opcional para el porcentaje de rechazo.
/// Elimina el archivo de log existente y crea una nueva instancia de `Gateway` para manejar conexiones.
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 2 {
        eprintln!("Usage: {} <rejection-percentage>", args[0]);
        std::process::exit(1);
    }

    let rejection_percentage: u8 = if args.len() < 2 {
        DEFAULT_REJECTION_PERCENTAGE
    } else {
        args[1].parse().expect("Invalid rejection_percentage")
    };

    if fs::metadata(LOG_FILE_PATH).is_ok() {
        fs::remove_file(LOG_FILE_PATH).expect("Failed to remove existing log file");
    }

    let log_file = LogFile::new(LOG_FILE_PATH);
    let gateway = Gateway::new(rejection_percentage, log_file);
    let gateway_address = format!("{}:{}", LOCAL_IP, GATEWAY_PORT);
    gateway.start(&gateway_address);
}
