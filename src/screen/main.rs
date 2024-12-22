extern crate actix;
use heladeria::screen::screen::Screen;
use std::{env, fs};

#[cfg(test)]
mod test;

/// Precondiciones de la Red de Screens:
///          - Tamaño mínimo: 2.
///          - Los nodos screen se instancian con los ID's en orden (0,1...,n).
///          - Se pueden instanciar después de haber levantado el anillo completo de robots y el gateway.
///
/// Punto de entrada principal para el binario `screen`.
/// Acepta tres argumentos: `server_id`, `total_servers` y `orders_path`.
/// Valida que `orders_path` sea un archivo JSON válido y luego crea e inicia una instancia de `Screen`.
///
/// Uso: cargo run --bin screen 0 3 (indicando screen id 0, screens totales en la red = 3)
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <server_id> <total_servers>", args[0]);
        std::process::exit(1);
    }

    let current_index: usize = args[1].parse().expect("Invalid current_index");
    let total_servers: usize = args[2].parse().expect("Invalid total_servers");
    let orders_path: &str = &args[3];

    // Check if orders_path is a valid JSON file
    if let Ok(contents) = fs::read_to_string(orders_path) {
        if let Ok(_json_value) = serde_json::from_str::<serde_json::Value>(&contents) {
            println!("JSON file is valid: {}", orders_path);
        } else {
            eprintln!("Error: {} is not a valid JSON file.", orders_path);
            std::process::exit(1);
        }
    } else {
        eprintln!("Error: Unable to read file {}", orders_path);
        std::process::exit(1);
    }

    let screen = Screen::new(current_index, total_servers, orders_path.to_string());
    screen.run(current_index, total_servers);
}
