use rand::Rng;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// Estructura para manejar el archivo de registro de logs.
pub struct LogFile {
    file: Arc<Mutex<std::fs::File>>,
}

impl LogFile {
    /// Crea una nueva instancia de `LogFile` abriendo o creando el archivo especificado.
    pub fn new(file_path: &str) -> Self {
        let file = Arc::new(Mutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path)
                .unwrap(),
        ));
        println!("Log file opened or created: {}", file_path);
        LogFile { file }
    }
    /// Escribe un mensaje en el archivo de log.
    fn write_log(&self, message: &str) {
        let mut file = self.file.lock().unwrap();
        writeln!(file, "{}", message).unwrap();
        println!("Log written: {}", message);
    }
}

/// Estructura para manejar el gateway que procesa órdenes y conexiones TCP.
pub struct Gateway {
    rejection_percentage: u8,
    log_file: LogFile,
    current_order_id: Arc<Mutex<u32>>,
    orders_table: Arc<Mutex<HashMap<u32, String>>>,
}

impl Gateway {
    const PREPARE: &'static str = "PREPARE";
    const COMMIT: &'static str = "COMMIT";
    const ABORT: &'static str = "ABORT";
    const ORDERS: &'static str = "ORDERS";

    /// Crea una nueva instancia de `Gateway` con un porcentaje de rechazo y un archivo de log.
    pub fn new(rejection_percentage: u8, log_file: LogFile) -> Self {
        Gateway {
            rejection_percentage,
            log_file,
            current_order_id: Arc::new(Mutex::new(1)),
            orders_table: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Inicia el servidor para escuchar conexiones TCP en la dirección especificada.
    pub fn start(&self, address: &str) {
        let listener = TcpListener::bind(address).unwrap();
        println!("Server listening on {}", address);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let gateway = self.clone();
                    thread::spawn(move || {
                        gateway.handle_connection(stream);
                    });
                }
                Err(e) => {
                    eprintln!("Connection failed: {}", e);
                }
            }
        }
    }

    /// Maneja el comando PREPARE, autorizando el pago y registrando la orden.
    fn handle_prepare(&self, parts: Vec<&str>, stream: &mut TcpStream) {
        let order_details = if parts.len() > 1 {
            parts[1..].join(",")
        } else {
            String::new()
        };

        if self.authorize_payment() {
            // Generate a new order ID
            let mut current_id = self.current_order_id.lock().unwrap();
            let new_order_id = *current_id;
            *current_id += 1;

            let mut orders_table = self.orders_table.lock().unwrap();
            orders_table.insert(new_order_id, order_details.clone());

            self.log_file.write_log(&format!(
                "{},{},{}",
                Self::PREPARE,
                new_order_id,
                order_details
            ));

            let response = format!("{},{}\n", Self::COMMIT, new_order_id);
            stream.write_all(response.as_bytes()).unwrap();
        } else {
            let response = format!("{}\n", Self::ABORT);
            stream.write_all(response.as_bytes()).unwrap();
            println!("Rejected: {}", order_details);
        }
    }
    /// Maneja el comando COMMIT, confirmando y removiendo la orden de la tabla.
    fn handle_commit(&self, parts: Vec<&str>, stream: &mut TcpStream) {
        let order_id = match parts[1].parse::<u32>() {
            Ok(id) => id,
            Err(_) => {
                let msg = "Invalid order ID\n";
                stream.write_all(msg.as_bytes()).unwrap();
                return;
            }
        };

        let mut orders_table = self.orders_table.lock().unwrap();
        if let Some(order_details) = orders_table.remove(&order_id) {
            self.log_file
                .write_log(&format!("{},{},{}", Self::COMMIT, order_id, order_details));
        } else {
            //let msg = "Order ID not found\n";
            //stream.write_all(msg.as_bytes()).unwrap();
        }
    }

    /// Maneja el comando ABORT, cancelando y removiendo la orden de la tabla.
    fn handle_abort(&self, parts: Vec<&str>, stream: &mut TcpStream) {
        let order_id = match parts[1].parse::<u32>() {
            Ok(id) => id,
            Err(_) => {
                let msg = "Invalid order ID\n";
                stream.write_all(msg.as_bytes()).unwrap();
                return;
            }
        };

        let mut orders_table = self.orders_table.lock().unwrap();
        if let Some(order_details) = orders_table.remove(&order_id) {
            self.log_file
                .write_log(&format!("{},{},{}", Self::ABORT, order_id, order_details));
        } else {
            //let msg = "Order ID not found\n";
            //stream.write_all(msg.as_bytes()).unwrap();
        }
    }
    /// Maneja el comando ORDERS, enviando la lista de órdenes actuales.
    fn handle_orders(&self, stream: &mut TcpStream) {
        let orders_table = self.orders_table.lock().unwrap();
        let mut response = String::from(Self::ORDERS);
        for (order_id, order_details) in orders_table.iter() {
            response.push_str(&format!(",{}:{}", order_id, order_details));
        }
        response.push('\n');
        stream.write_all(response.as_bytes()).unwrap();
    }
    /// Maneja comandos desconocidos enviando un mensaje de error al cliente.
    fn handle_unknown_command(&self, stream: &mut TcpStream) {
        let msg = "Unknown command\n";
        stream.write_all(msg.as_bytes()).unwrap();
    }
    /// Maneja una conexión TCP, leyendo y procesando mensajes en un bucle.
    fn handle_connection(&self, mut stream: TcpStream) {
        loop {
            let mut buffer = [0; 1024];
            let bytes_read = match stream.read(&mut buffer) {
                Ok(bytes) if bytes == 0 => return,
                Ok(bytes) => bytes,
                Err(_) => return,
            };

            let message = String::from_utf8_lossy(&buffer[..bytes_read]);
            let parts: Vec<&str> = message.trim().split(',').collect();
            if parts.is_empty() {
                let msg = "Invalid message format\n";
                stream.write_all(msg.as_bytes()).unwrap();
                continue;
            }
            let mut index = 0;
            while index < parts.len() {
                let mut segment_end = index + 1;

                // Find where the current command segment ends
                while segment_end < parts.len() && !Self::is_command(parts[segment_end]) {
                    segment_end += 1;
                }

                // Process the current segment
                self.handle_message((&parts[index..segment_end]).to_vec(), &mut stream);

                // Move index to the next segment
                index = segment_end;
            }
        }
    }
    /// Determina si una cadena es un comando válido.
    fn is_command(part: &str) -> bool {
        part == Self::PREPARE || part == Self::COMMIT || part == Self::ABORT || part == Self::ORDERS
    }
    /// Maneja un mensaje específico en función del comando recibido.
    fn handle_message(&self, parts: Vec<&str>, stream: &mut TcpStream) {
        let command = parts[0];
        match command {
            Self::PREPARE => self.handle_prepare(parts, stream),
            Self::COMMIT => self.handle_commit(parts, stream),
            Self::ABORT => self.handle_abort(parts, stream),
            Self::ORDERS => self.handle_orders(stream),
            _ => self.handle_unknown_command(stream),
        };
    }
    /// Autoriza el pago generando un número aleatorio y comparándolo con el porcentaje de rechazo.
    fn authorize_payment(&self) -> bool {
        let rand_num: u8 = rand::thread_rng().gen_range(0..=100);
        rand_num > self.rejection_percentage
    }
}

impl Clone for Gateway {
    /// Implementa el clon de `Gateway` para permitir su uso en múltiples hilos.
    fn clone(&self) -> Self {
        Gateway {
            rejection_percentage: self.rejection_percentage,
            log_file: LogFile {
                file: Arc::clone(&self.log_file.file),
            },
            current_order_id: Arc::clone(&self.current_order_id),
            orders_table: Arc::clone(&self.orders_table),
        }
    }
}
