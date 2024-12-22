// use crate::actix::Actor;
// use crate::order_table::OrderTable;
// use crate::screen_actors::AbortOrder;
// use crate::screen_actors::CommitOrder;
// use crate::screen_actors::FileReaderActor;
// use crate::screen_actors::ProcessorActor;
// use crate::screen_actors::ReadFile;
// use crate::screen_actors::ScreenActor;
// use crate::utils::*;
use actix::{Actor, System};
// use heladeria::common::constants::GATEWAY_ADDRESS;
// use heladeria::common::messages::*;
use std::collections::HashMap;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self};
use std::time::Duration;

use crate::common::constants::GATEWAY_ADDRESS;
use crate::common::messages::*;
use crate::screen::utils::handle_incoming_connection;

use super::order_table::OrderTable;
use super::screen_actors::{
    AbortOrder, Address, CommitOrder, FileReaderActor, ProcessorActor, ReadFile, ScreenActor,
};
/// La estructura `Screen` representa una pantalla en la red.
/// Contiene información sobre el liderazgo, identificación,
/// tamaño de la red, tabla de pedidos y canales de comunicación.
pub struct Screen {
    pub is_leader: Arc<Mutex<bool>>,
    pub id: usize,
    pub robot_leader_id: Arc<Mutex<usize>>,
    pub leader_id: Arc<Mutex<usize>>,
    pub network_size: Arc<Mutex<usize>>,
    pub order_table: Arc<Mutex<OrderTable>>,
    pub network: Arc<Mutex<HashMap<usize, usize>>>,
    pub rx_sender_channel: Arc<Mutex<Receiver<MessageType>>>,
    pub tx_sender_channel: Arc<Sender<MessageType>>,
    pub rx_prepare_channel: Arc<Mutex<Receiver<MessageType>>>,
    pub tx_prepare_channel: Arc<Sender<MessageType>>,
    pub rx_robot_sender_channel: Arc<Mutex<Option<Receiver<MessageType>>>>,
    pub tx_robot_sender_channel: Arc<Sender<MessageType>>,
    pub orders_path: String,
}
/// Implementación de la estructura `Screen`.
/// Contiene funciones para inicializar, manejar y depurar la red y los actores.
impl Screen {
    /// Crea una nueva instancia de `Screen` con el ID, tamaño de red y ruta de pedidos especificados.
    pub fn new(id: usize, network_size: usize, orders_path: String) -> Screen {
        let (tx, rx) = mpsc::channel();
        let rx_sender_channel = Arc::new(Mutex::new(rx));
        let tx_sender_channel = Arc::new(tx);

        let (tx1, rx1) = mpsc::channel();
        let rx_prepare_channel = Arc::new(Mutex::new(rx1));
        let tx_prepare_channel = Arc::new(tx1);

        let (tx3, rx3) = mpsc::channel(); // Crear canales async
        let rx_robot_sender_channel = Arc::new(Mutex::new(Some(rx3)));
        let tx_robot_sender_channel = Arc::new(tx3);

        Screen {
            is_leader: Arc::new(Mutex::new(id == 0)),
            id,
            leader_id: Arc::new(Mutex::new(0)), // inicialmente, screen 0 es el lider.
            robot_leader_id: Arc::new(Mutex::new(0)), // inicialmente, robot 0 es el lider.
            order_table: Arc::new(Mutex::new(OrderTable::new())),
            network_size: Arc::new(Mutex::new(network_size)),
            network: Arc::new(Mutex::new(HashMap::new())),
            rx_sender_channel,
            tx_sender_channel,
            rx_prepare_channel,
            tx_prepare_channel,
            rx_robot_sender_channel,
            tx_robot_sender_channel,
            orders_path,
        }
    }

    /// Inicializa la red, conectando las pantallas en un anillo.
    pub fn initialize_network(&self) {
        let mut net = self.network.lock().unwrap();
        let n = *self.network_size.lock().unwrap();
        net.insert(0, n - 1);
        net.insert(1, 0);
        for i in 2..n {
            net.insert(i, i - 1);
        }
    }

    /// Imprime el estado de la red.
    pub fn print_network(&self) {
        let net = self.network.lock().unwrap();
        for (node, &connected_node) in net.iter() {
            println!("Screen {} is connected to Screen {}", node, connected_node);
        }
    }

    /// Inicializa los actores necesarios y gestiona la recepción y procesamiento de mensajes.
    pub async fn initialize_actors(self: Arc<Self>, rx: mpsc::Receiver<MessageType>) {
        // Crear ProcessorActor
        let gateway_address = GATEWAY_ADDRESS.to_string();
        let processor_actor =
            match ProcessorActor::new(self.id, gateway_address, self.tx_sender_channel.clone()) {
                Ok(actor) => actor.start(),
                Err(err) => {
                    eprintln!("Failed to create ProcessorActor: {}", err);
                    std::process::exit(1);
                }
            };

        // Crear ScreenActor
        let screen_actor = ScreenActor::new(processor_actor.clone().recipient()).start();

        // Crear FileReaderActor
        let file_reader_actor = FileReaderActor::new(screen_actor.clone().recipient()).start();

        // Ejemplo de cómo enviar un mensaje al FileReaderActor
        file_reader_actor.do_send(ReadFile::new(self.orders_path.clone()));
        //let mut rx = self.rx_robot_sender_channel.lock().unwrap();

        thread::spawn(move || {
            loop {
                processor_actor.do_send(Address {
                    screen_addr: screen_actor.clone(),
                });
                // Recibir el mensaje del canal
                let message = match rx.recv() {
                    Ok(message) => message,
                    Err(_) => {
                        println!("Error al recibir mensaje. Saliendo del bucle.");
                        break;
                    }
                };

                // Procesar el mensaje según su tipo
                match message {
                    MessageType::Commit(commit_order) => {
                        let commit_order_msg = CommitOrder {
                            order_id: commit_order.order_id as u32,
                        };
                        processor_actor.do_send(commit_order_msg);
                    }
                    MessageType::Abort(abort_order) => {
                        let abort_order_msg = AbortOrder {
                            order_id: abort_order.order_id as u32,
                        };
                        processor_actor.do_send(abort_order_msg);
                    }
                    _ => {
                        println!("Mensaje no reconocido recibido en rx_robot_sender_channel.");
                    }
                }
            }
        });
    }
    /// Devuelve el canal de mensajes del robot.
    pub fn get_robot_channel(&self) -> mpsc::Receiver<MessageType> {
        let mut rx = self.rx_robot_sender_channel.lock().unwrap();
        let channel = rx.take().expect("rx_robot_sender_channel is None");
        *rx = None;
        channel
    }
    /// Inicia la gestión de pedidos en un nuevo hilo, ejecutando el sistema Actix y los actores necesarios.
    pub fn start_orders(self: Arc<Self>, rx: mpsc::Receiver<MessageType>) {
        let arc_order = self.clone();
        let _system_thread_handle = thread::spawn(move || {
            let system = System::new();
            system.block_on(async move {
                // Ejecutar la lógica principal de Screen
                arc_order.initialize_actors(rx).await;
            });

            // Ejecutar el sistema Actix en el hilo actual
            system.run().unwrap();
        });
    }
    /// Inicializa y ejecuta la pantalla, configurando la red y comenzando a escuchar conexiones.
    pub fn run(self, my_id: usize, network_size: usize) {
        let arc_self = Arc::new(self);
        arc_self.initialize_network();
        arc_self.print_network();
        let am_i_leader = my_id == 0;

        let arc_tx = arc_self.clone();
        if !am_i_leader {
            thread::spawn(move || {
                let robot_intro_msg =
                    MessageType::ScreenIntroduction(ScreenIntroduction { sender_id: my_id });
                arc_tx.tx_sender_channel.send(robot_intro_msg).unwrap();

                // Si soy el ultimo robot, avisar a la red que ya estan todos los robots activos.
                if my_id == network_size - 1 {
                    let allconnected_msg =
                        MessageType::AllConnected(AllConnected { sender_id: my_id });
                    arc_tx.tx_sender_channel.send(allconnected_msg).unwrap();
                }

                arc_tx.connect_to_next_screen();
            });
        }

        if my_id == network_size - 1 {
            arc_self.clone().start_orders(arc_self.get_robot_channel());
        }

        // Ejemplo de llamada a start_listener desde Arc<Self>
        arc_self.start_listener();
    }
    /// Comienza a escuchar conexiones TCP entrantes y maneja cada conexión en un hilo separado.
    pub fn start_listener(self: Arc<Self>) {
        println!("Screen ID {} online", self.id);
        let my_id = self.id.clone();
        let listener_addr = format!("127.0.0.1:{}", 3000 + my_id);
        let listener = TcpListener::bind(&listener_addr).expect("Failed to bind");
        let listener_clone = listener.try_clone().expect("Failed to clone listener");

        for stream in listener_clone.incoming() {
            match stream {
                Ok(socket) => {
                    // Incoming connection
                    println!(
                        "Screen {}: Accepted connection from {}",
                        my_id,
                        socket.peer_addr().unwrap()
                    );

                    let nested_thread_self = Arc::clone(&self);
                    thread::spawn(move || {
                        handle_incoming_connection(nested_thread_self, socket, my_id)
                    });
                }
                Err(e) => {
                    eprintln!("{}", e);
                }
            }
        }
    }
    /// Conecta la pantalla líder con el robot líder y maneja la comunicación entre ellos.
    pub fn connect_robot(self: Arc<Self>, is_connected: bool) {
        println!("Soy el lider. Tengo que intercambiar mensajes con robot lider...");
        let rx_prepare_channel = self.rx_prepare_channel.clone();
        let tx_prepare_channel = self.tx_prepare_channel.clone();

        // Intentar conectar en puertos del 10000 al 10009
        // let socket = (10000..10010).find_map(|port| {
        //     let addr: SocketAddr = format!("127.0.0.1:{}", port)
        //         .parse()
        //         .expect("Invalid address");

        //     match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
        //         Ok(sock) => {
        //             println!("Conectado exitosamente al puerto {}", port);
        //             Some(sock)
        //         }
        //         Err(e) => {
        //             eprintln!("Error al conectar al puerto {}: {}", port, e);
        //             None
        //         }
        //     }
        // });

        let robot_id = self.robot_leader_id.lock().unwrap();
        let robot_addr = format!("127.0.0.1:{}", 10000 + *robot_id);
        drop(robot_id);

        //let addr = format!("127.0.0.1:{}", 10000 + robot_id);
        let socket = match TcpStream::connect(robot_addr) {
            Ok(sock) => Some(sock),
            Err(e) => {
                eprintln!("Unable to connect to robot: {:?}", e);
                None
            }
        };

        // Si no se pudo conectar a ningún puerto, salir del método
        let mut socket = match socket {
            Some(socket) => socket,
            None => {
                eprintln!(
                    "No se pudo conectar a ninguno de los puertos especificados. Saliendo..."
                );
                return;
            }
        };

        let mut intro_msg = MessageType::ScreenIntroduction(ScreenIntroduction {
            sender_id: self.id, // Acceder al sender_id desde self
        });

        if is_connected {
            intro_msg = MessageType::NewLeaderIntroduction(self.id);
        }

        let serialized = serialize_message(&intro_msg).expect("Failed to serialize message");
        let _ = socket.write_all(&serialized);

        thread::spawn(move || loop {
            match rx_prepare_channel.lock().unwrap().recv() {
                Ok(message) => match message {
                    MessageType::Order(ref _prepare_order) => {
                        let serialized =
                            serialize_message(&message).expect("Failed to serialize message");

                        if let Err(e) = socket.write_all(&serialized) {
                            eprintln!("Error al enviar por el socket: {}", e);
                            //self.connect_robot();
                            let _ = tx_prepare_channel.send(message);
                            break;
                        }
                    }
                    MessageType::Kill() => {
                        println!("Kill recibido. Cierro thread");
                        //self.connect_robot();
                        break;
                    }
                    _ => {
                        println!("Mensaje no reconocido en el canal prepare orders.");
                    }
                },
                Err(_) => {
                    println!("Canal cerrado. Terminando hilo de envío al socket.");
                    break;
                }
            }
        });
    }

    /// Devuelve el ID de la siguiente pantalla a la que se debe conectar.
    pub fn find_next_id(&self) -> Option<usize> {
        let network_lock = self.network.lock().unwrap();
        network_lock.get(&self.id).copied()
    }
    /// Actualiza el líder de la red y establece si el ID actual es el nuevo líder.
    pub fn set_new_leader(&self, my_id: usize, new_leader_id: usize) {
        let mut id = self.leader_id.lock().unwrap();
        *id = new_leader_id;
        println!("Nuevo lider en la red: {}", new_leader_id);

        if my_id == new_leader_id {
            let mut is_leader = self.is_leader.lock().unwrap();
            *is_leader = true;
        }
    }
    /// Establece un nuevo líder para los robots en la red.
    pub fn set_new_robot_leader(&self, new_robot_leader_id: usize) {
        let mut id: std::sync::MutexGuard<usize> = self.robot_leader_id.lock().unwrap();
        *id = new_robot_leader_id;
    }

    /// Devuelve `true` si el ID especificado está conectado a este nodo.
    pub fn is_connected_to_me(&self, dead_screen_id: usize) -> bool {
        let network = self.network.lock().unwrap();
        let my_id = self.id;

        if let Some(existing_value) = network.get(&my_id) {
            return *existing_value == dead_screen_id;
        }

        false
    }
    /// Conecta a la siguiente pantalla en la red y maneja la comunicación entre pantallas.
    pub fn connect_to_next_screen(&self) {
        let tx_sender_channel: Arc<Sender<MessageType>> = self.tx_sender_channel.clone();
        let next_screen_id = self.find_next_id();

        match next_screen_id {
            Some(next_id) => {
                let next_addr = format!("127.0.0.1:{}", 3000 + next_id);
                match TcpStream::connect(&next_addr) {
                    Ok(mut stream) => {
                        println!(
                            "Screen {}: Connected to next screen id: {}",
                            self.id, next_id
                        );
                        let rx_sender_channel = self.rx_sender_channel.lock().unwrap();
                        loop {
                            match rx_sender_channel.recv() {
                                Ok(message) => {
                                    match message {
                                        MessageType::Kill() => {
                                            println!(
                                                "Received KILL command. Terminating sender thread."
                                            );
                                            break;
                                        }
                                        _ => {
                                            thread::sleep(Duration::from_millis(1));
                                            let serialized = serialize_message(&message)
                                                .expect("Failed to serialize message");

                                            if let Err(e) = stream.write_all(&serialized) {
                                                // Falla enviar por el socket
                                                // mandamos por canal para que se vuelva a procesar (en el nuevo sender)
                                                let _ = tx_sender_channel.send(message);
                                                eprintln!(
                                            "screen {}: Failed to write to stream; err = {:?}",
                                            self.id, e
                                        );
                                                return;
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    println!("Channel closed");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to Connect to next screen: {}", e);
                    }
                }
            }
            None => {
                eprintln!("Error: Next screen not found.");
            }
        }
    }

    /// Devuelve `true` si el ID especificado es el líder de la red.
    pub fn is_leader(&self, id: usize) -> bool {
        let leader = self.leader_id.lock().unwrap();
        return *leader == id;
    }
    /// Devuelve `true` si este nodo es el líder de la red.
    pub fn i_am_leader(&self) -> bool {
        let is_leader = self.is_leader.lock().unwrap();
        *is_leader
    }

    /// Devuelve el ID del nodo del que este nodo está leyendo en la red.
    pub fn find_prev_screen(&self, self_id: usize) -> Option<usize> {
        let network_lock = self.network.lock().unwrap();
        let mut reading_from_id: Option<usize> = None;
        for (key, value) in &*network_lock {
            if value == &self_id {
                reading_from_id = Some(*key);
            }
        }

        return reading_from_id;
    }
    /// Envía un mensaje de elección de líder a través del canal de envío.
    pub fn send_leader_election_message(&self, id: usize, dead_leader_id: usize) {
        let election_msg = MessageType::Election(Election {
            sender_id: id,
            current_candidate_id: id,
            dead_leader_id: dead_leader_id,
        });
        let _ = self.tx_sender_channel.send(election_msg);
    }
    /// Aplica una orden recibida, añadiéndola a la tabla de órdenes y, si es líder, enviando un mensaje de orden.
    pub fn apply_order(self: Arc<Self>, order: &OrderScreen) {
        let detalle = &order.order_details;
        let mut order_table = self.order_table.lock().unwrap();
        order_table.add_order(
            order.sender_id as i32,
            order.order_id as i32,
            detalle.clone(),
        );
        if self.i_am_leader() {
            let order_msg = MessageType::Order(Order {
                order_id: order.order_id,
                order_details: detalle.clone(),
            });
            let _ = self.tx_prepare_channel.send(order_msg);
        }
    }
    /// Aborta una orden especificada, eliminándola de la tabla de órdenes y enviando un mensaje de abortar si es necesario.
    pub fn abort_order(self: Arc<Self>, abort: &Abort) {
        let mut order_table = self.order_table.lock().unwrap();
        let order_id = abort.order_id as i32;
        if let Some((screen_id, details)) = order_table.remove_order(order_id) {
            // Verificar si el screen_id es igual a self.id
            if screen_id == self.id as i32 {
                let msg = MessageType::Abort(Abort {
                    order_id: abort.order_id,
                });
                self.tx_robot_sender_channel.send(msg).unwrap();
                println!("Pedido {} ABORTADO:", order_id);
                for (flavor, quantity) in details {
                    println!("  - {}: {}", flavor, quantity);
                }
            }
        }
    }
    /// Confirma una orden especificada, eliminándola de la tabla de órdenes y enviando un mensaje de commit si es necesario.
    pub fn commit_order(self: Arc<Self>, commit: &Commit) {
        let mut order_table = self.order_table.lock().unwrap();
        let order_id = commit.order_id as i32;
        if let Some((screen_id, details)) = order_table.remove_order(order_id) {
            // Verificar si el screen_id es igual a self.id
            if screen_id == self.id as i32 {
                let msg = MessageType::Commit(Commit {
                    order_id: commit.order_id,
                });
                self.tx_robot_sender_channel.send(msg).unwrap();
                println!("Pedido {} TERMINADO:", order_id);
                for (flavor, quantity) in details {
                    println!("  - {}: {}", flavor, quantity);
                }
            }
        }
    }
    /// Transfiere las órdenes de un nodo muerto a otro nodo especificado.
    pub fn transfer_orders(&self, dead_id: usize, transfer_id: usize) {
        let mut order_table = self.order_table.lock().unwrap();
        let changed_orders = order_table.transfer_orders(dead_id as i32, transfer_id as i32);
        if transfer_id == self.id {
            println!("Recibo los pedidos: {:?}", changed_orders);
        }
    }
    /// Actualiza la red eliminando un nodo muerto y actualizando las conexiones de los nodos restantes.
    pub fn update_network(screen: Arc<Self>, dead_id_u: usize) {
        // Reducir el tamano de la red
        let mut net_size = screen.network_size.lock().unwrap();
        *net_size -= 1;

        //let dead_id_u: usize = dead_id.parse().expect("Not a valid usize");
        let mut network = screen.network.lock().unwrap();

        // el id del screen que leia del screen que murio
        let mut _id_connected_to_dead: Option<usize> = None;

        // el id del screen que le escribia al screen que murio
        let mut dead_connected_to_id: Option<usize> = None;

        for (key, value) in &*network {
            if value == &dead_id_u {
                dead_connected_to_id = Some(*key);
                break;
            }
        }

        // id del screen que leia del screen que murio
        _id_connected_to_dead = network.get(&dead_id_u).copied();

        // eliminar screen muerto de la tabla
        network.remove(&dead_id_u);

        // Actualizar la tabla:
        // screen que le escribia al que murio (dead_conected_to_id) ahora le escribe al id_connected_to_dead
        if let (Some(dead_id), Some(new_id)) = (dead_connected_to_id, _id_connected_to_dead) {
            // Use get_mut with the dereferenced dead_id
            *network.get_mut(&dead_id).unwrap() = new_id;
        }

        println!("Updated network: {:?}", network); // debug print
    }
}
