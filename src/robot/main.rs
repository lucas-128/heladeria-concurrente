use heladeria::robot::robot::Robot;
use std::env;

/// Uso: cargo run --bin robot <robot_id> <total_servers>
///
/// Este programa instancia un robot dentro de una red de robots.
///
/// Precondiciones de la red:
/// - Los robots se instancian con IDs en orden (0, 1, ..., n).
///
/// # Argumentos
///
/// - `robot_id`: Identificador único del robot actual.
/// - `total_servers`: Total de robots en la red.
///
/// # Ejemplo
///
/// Para ejecutar el primer robot de una red de 3 robots:
///
/// cargo run --bin robot 0 3
///
/// Esto inicializa el primer robot con ID 0 en una red de 3 robots.
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <server_id> <total_servers>", args[0]);
        std::process::exit(1);
    }

    let current_index: usize = args[1].parse().expect("Invalid current_index");
    let total_servers: usize = args[2].parse().expect("Invalid total_servers");

    let robot = Robot::new(current_index, total_servers);
    robot.run(current_index, total_servers);
}
