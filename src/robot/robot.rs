/// Este módulo define la estructura `Robot` y su implementación, incluyendo métodos
/// para gestionar conexiones de red, manejar pedidos e interactuar con otros componentes
/// del sistema.
///
use super::robot_orders_table::{OrdersList, OrdersTable};
use super::stock_table::Stock;
use crate::common::constants::{INITIAL_GRAMS_AMOUNT, SLEEP_FACTOR};
use crate::common::flavors::{FlavorInfo, IceCreamFlavor};
use crate::common::messages::*;
use crate::robot::utils::handle_incoming_connection;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::{self};
use std::time::{Duration, SystemTime};

/// Representa un nodo robot en el sistema distribuido.
pub struct Robot {
    pub is_leader: Arc<Mutex<bool>>,
    pub id: usize,
    pub tokens_table: Arc<Mutex<HashMap<IceCreamFlavor, FlavorInfo>>>,
    pub screen_leader_id: Arc<Mutex<usize>>,
    pub leader_id: Arc<Mutex<usize>>,
    pub stock_table: Arc<Mutex<Stock>>,
    pub network_size: Arc<Mutex<usize>>,
    pub orders_table: Arc<Mutex<OrdersTable>>,
    pub network: Arc<Mutex<HashMap<usize, usize>>>,
    pub rx_sender_channel: Arc<Mutex<Receiver<MessageType>>>,
    pub tx_sender_channel: Arc<Sender<MessageType>>,
    pub rx_prepare_channel: Arc<Mutex<Receiver<MessageType>>>,
    pub tx_prepare_channel: Arc<Sender<MessageType>>,
    pub rx_token_channel: Arc<Mutex<Receiver<MessageType>>>,
    pub tx_token_channel: Arc<Sender<MessageType>>,
    pub rx_screen_sender_channel: Arc<Mutex<Receiver<MessageType>>>,
    pub tx_screen_sender_channel: Arc<Sender<MessageType>>,
}

impl Robot {
    /// Método constructor para `Robot`.
    pub fn new(id: usize, network_size: usize) -> Robot {
        let (tx, rx) = unbounded();
        let rx_sender_channel = Arc::new(Mutex::new(rx));
        let tx_sender_channel = Arc::new(tx);

        let (tx1, rx1) = unbounded();
        let rx_prepare_channel = Arc::new(Mutex::new(rx1));
        let tx_prepare_channel = Arc::new(tx1);

        let (tx2, rx2) = unbounded();
        let rx_token_channel = Arc::new(Mutex::new(rx2));
        let tx_token_channel = Arc::new(tx2);

        let (tx3, rx3) = unbounded();
        let rx_screen_sender_channel = Arc::new(Mutex::new(rx3));
        let tx_screen_sender_channel = Arc::new(tx3);

        Robot {
            is_leader: Arc::new(Mutex::new(id == 0)),
            id,
            screen_leader_id: Arc::new(Mutex::new(0)), // inicialmente, pantalla 0 es el lider.
            leader_id: Arc::new(Mutex::new(0)),        // inicialmente, robot 0 es el lider.
            network_size: Arc::new(Mutex::new(network_size)),
            orders_table: Arc::new(Mutex::new(OrdersTable::new())),
            network: Arc::new(Mutex::new(HashMap::new())),
            stock_table: Arc::new(Mutex::new(Stock::new())),
            tokens_table: Arc::new(Mutex::new(HashMap::new())),
            rx_sender_channel,
            tx_sender_channel,
            rx_prepare_channel,
            tx_prepare_channel,
            rx_token_channel,
            tx_token_channel,
            rx_screen_sender_channel,
            tx_screen_sender_channel,
        }
    }

    /// Inicializa las conexiones de red.
    pub fn initialize_network(&self, n: usize) {
        let mut net = self.network.lock().unwrap();
        net.insert(0, n - 1);
        net.insert(1, 0);
        for i in 2..n {
            net.insert(i, i - 1);
        }
    }

    /// Función de depuración para imprimir las conexiones de red actuales.
    pub fn print_network(&self) {
        let net = self.network.lock().unwrap();
        for (node, &connected_node) in net.iter() {
            println!("Robot {} is connected to Robot {}", node, connected_node);
        }
    }

    /// Actualiza la tabla de tokens basado en cantidades leídas y usadas.
    pub fn update_tokens_table(
        &self,
        flavor: &IceCreamFlavor,
        read_ammount: i32,
        used_ammount: i32,
    ) {
        let now = SystemTime::now();
        let mut table = self.tokens_table.lock().unwrap();
        if let Some(flavor_info) = table.get_mut(&flavor) {
            flavor_info.stock = read_ammount - used_ammount;
            flavor_info.last_modification_timestamp = now;
        }
    }

    /// Verifica si un timestamp dado es mayor que el último timestamp de modificación de un sabor.
    pub fn is_timestamp_greater(&self, flavor: &IceCreamFlavor, timestamp: SystemTime) -> bool {
        let table = self.tokens_table.lock().unwrap();
        if let Some(info) = table.get(flavor) {
            return info.last_modification_timestamp > timestamp;
        } else {
            return false;
        }
    }

    /// Inicia el robot, inicializando componentes necesarios y comenzando listeners y handlers.
    pub fn run(self, my_id: usize, network_size: usize) {
        let arc_self: Arc<Robot> = Arc::new(self);
        arc_self.initialize_network(network_size);
        //arc_self.print_network(); // Debug
        arc_self.initialize_orders_table(network_size);
        arc_self.initialize_token_table();

        let am_i_leader = my_id == 0;
        let arc_tx_1 = arc_self.clone();
        let arc_tx_2 = arc_self.clone();

        thread::spawn(move || {
            arc_tx_1.start_order_handler();
        });

        if !am_i_leader {
            thread::spawn(move || {
                if my_id == network_size - 1 {
                    let allconnected_msg =
                        MessageType::AllConnected(AllConnected { sender_id: my_id });
                    let _ = arc_tx_2.tx_sender_channel.send(allconnected_msg);
                }

                arc_tx_2.connect_to_next_robot();
            });
        }

        arc_self.start_listener();
    }

    /// Inicializa la tabla de tokens con valores iniciales.
    pub fn initialize_token_table(&self) {
        let flavors = IceCreamFlavor::iter();
        let mut table = self.tokens_table.lock().unwrap();

        for flavor in flavors {
            table.insert(
                flavor.clone(),
                FlavorInfo {
                    has_token: false,
                    stock: INITIAL_GRAMS_AMOUNT,
                    last_modification_timestamp: SystemTime::now(),
                },
            );
        }
    }

    /// Establece el estado del token para un sabor específico con la cantidad disponible,
    /// el último modificado por y el timestamp de última modificación.
    pub fn set_token_status(&self, flavor: IceCreamFlavor, status: bool) {
        let mut table = self.tokens_table.lock().unwrap();
        if let Some(info) = table.get_mut(&flavor) {
            info.has_token = status;
        }
    }

    /// Comprueba si existe un token para el sabor especificado.
    pub fn has_token(&self, flavor: &IceCreamFlavor) -> bool {
        let table = self.tokens_table.lock().unwrap();
        if let Some(info) = table.get(flavor) {
            info.has_token
        } else {
            false
        }
    }

    /// Obtiene el stock disponible para un sabor de helado específico.
    pub fn get_flavor_stock(&self, flavor: &IceCreamFlavor) -> i32 {
        let table = self.tokens_table.lock().unwrap();
        if let Some(value) = table.get(flavor).map(|info| info.stock) {
            return value;
        } else {
            return 0;
        }
    }

    /// Conecta al robot con la pantalla líder en la red.
    pub fn connect_to_screen(&self, is_new_leader_introduction: bool) {
        let screen_id = self.screen_leader_id.lock().unwrap();
        let tx_sender_screen = self.tx_screen_sender_channel.clone();
        let screen_addr = format!("127.0.0.1:{}", 3000 + *screen_id);
        drop(screen_id);

        match TcpStream::connect(&screen_addr) {
            Ok(mut stream) => {
                let mut robot_intro_msg =
                    MessageType::RobotIntroduction(RobotIntroduction { sender_id: self.id });

                if is_new_leader_introduction {
                    robot_intro_msg = MessageType::NewLeaderIntroduction(self.id);
                }

                let serialized =
                    serialize_message(&robot_intro_msg).expect("Failed to serialize message");

                let _ = stream.write_all(&serialized);

                let rx_screen = self.rx_screen_sender_channel.lock().unwrap();
                loop {
                    match rx_screen.recv() {
                        Ok(message) => match message {
                            MessageType::Kill() => {
                                break;
                            }
                            _ => {
                                thread::sleep(Duration::from_millis(1));
                                let serialized = serialize_message(&message)
                                    .expect("Failed to serialize message");

                                if let Err(_) = stream.write_all(&serialized) {
                                    let _ = tx_sender_screen.send(message);
                                    break;
                                }
                            }
                        },
                        _ => {}
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to Connect to Screen: {}", e);
            }
        };
    }

    /// Establece la nueva pantalla líder de la red.
    pub fn set_screen_leader(&self, screen_leader_id: usize) {
        let mut leader_id = self.screen_leader_id.lock().unwrap();
        *leader_id = screen_leader_id;
    }

    /// Inicia el listener para aceptar conexiones entrantes.
    pub fn start_listener(self: Arc<Self>) {
        println!("Robot ID {} online", self.id);
        let my_id = self.id.clone();
        let listener_addr = format!("127.0.0.1:{}", 10000 + my_id);
        let listener = TcpListener::bind(&listener_addr).expect("Failed to bind");
        let listener_clone = listener.try_clone().expect("Failed to clone listener");

        for stream in listener_clone.incoming() {
            match stream {
                Ok(socket) => {
                    // Incoming connection
                    // println!(
                    //     "Robot {}: Accepted connection from {}",
                    //     my_id,
                    //     socket.peer_addr().unwrap()
                    // );

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

    /// Inicializa la tabla de órdenes con un tamaño de red específico.
    pub fn initialize_orders_table(&self, network_size: usize) {
        self.orders_table.lock().unwrap().initialize(network_size);
    }

    /// Verifica si hay suficiente stock disponible para los detalles del pedido especificados
    pub fn has_enough_stock(&self, order_details: HashMap<IceCreamFlavor, i32>) -> bool {
        self.stock_table
            .lock()
            .unwrap()
            .has_enough_stock(&order_details)
    }

    /// Resta el stock especificado de los detalles del pedido de la tabla de stock.
    pub fn subtract_stock(&self, order_details: HashMap<IceCreamFlavor, i32>) -> SystemTime {
        self.stock_table
            .lock()
            .unwrap()
            .subtract_stock(&order_details)
    }

    /// Agrega el stock especificado a los detalles del pedido en la tabla de stock.
    pub fn add_stock(&self, order_details: HashMap<IceCreamFlavor, i32>) -> SystemTime {
        self.stock_table.lock().unwrap().add_stock(&order_details)
    }

    /// Agrega el stock especificado con una marca de tiempo a los detalles del pedido en la tabla de stock.
    pub fn add_stock_with_timestamp(
        &self,
        order_details: HashMap<IceCreamFlavor, i32>,
        timestamp: SystemTime,
    ) {
        self.stock_table
            .lock()
            .unwrap()
            .add_with_timestamp(&order_details, timestamp);
    }

    /// Resta el stock especificado con una marca de tiempo de los detalles del pedido en la tabla de stock.
    pub fn subtract_stock_with_timestamp(
        &self,
        order_details: HashMap<IceCreamFlavor, i32>,
        timestamp: SystemTime,
    ) {
        self.stock_table
            .lock()
            .unwrap()
            .subtract_with_timestamp(&order_details, timestamp);
    }

    /// Maneja la recepción y procesamiento de pedidos de helado.
    pub fn start_order_handler(self: Arc<Self>) {
        let tx_sender = self.tx_sender_channel.clone();
        let rx_prepare = self.rx_prepare_channel.lock().unwrap().clone();
        let rx_token = self.rx_token_channel.lock().unwrap().clone();
        let token_in_use = Arc::new(Mutex::new(false));
        let mut thread_handles = Vec::new();

        loop {
            select! {
                recv(rx_prepare) -> message => {
                    match message {
                        Ok(MessageType::Prepare(order)) => {
                            println!("Preparando pedido ID: {}", order.order_id);
                            println!("Detalles del pedido: {:?}", order.order_details);
                            let mut required_flavours: Vec<IceCreamFlavor> = order.order_details.keys().cloned().collect();

                            while required_flavours.len() > 0 {
                                select! {
                                    recv(rx_token) -> token_message => {
                                        match token_message {
                                            Ok(MessageType::Token(token)) => {
                                                if required_flavours.contains(&token.flavour) {
                                                    if let Some(&amount) = order.order_details.get(&token.flavour) {

                                                        self.update_tokens_table(&token.flavour.clone(), token.available_ammount.clone(), amount);

                                                        let tx_sender_clone = tx_sender.clone();
                                                        let self_clone = Arc::clone(&self);
                                                        let token_in_use_clone = Arc::clone(&token_in_use);

                                                        let mut token_in_use_guard = token_in_use.lock().unwrap();
                                                        if !*token_in_use_guard {
                                                            *token_in_use_guard = true;

                                                            let flavour_clone = token.flavour.clone();


                                                            let handle = thread::spawn(move || {
                                                                let used_token = self_clone.use_token(token, amount);
                                                                self_clone.set_token_status(used_token.flavour.clone(), false);
                                                                let _ = tx_sender_clone.send(MessageType::Token(used_token));


                                                                let mut token_in_use_guard = token_in_use_clone.lock().unwrap();
                                                                *token_in_use_guard = false;
                                                            });


                                                            thread_handles.push(handle);
                                                            required_flavours.retain(|x| x != &flavour_clone);

                                                        } else {

                                                            self.set_token_status(token.flavour.clone(), false);
                                                            self.update_tokens_table(&token.flavour.clone(), token.available_ammount.clone(), 0);
                                                            let _ = tx_sender.send(MessageType::Token(token));
                                                        }
                                                    }
                                                } else {

                                                    self.set_token_status(token.flavour.clone(), false);
                                                    self.update_tokens_table(&token.flavour.clone(), token.available_ammount.clone(), 0);
                                                    let _ = tx_sender.send(MessageType::Token(token));
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }


                            for handle in thread_handles.drain(..) {
                                handle.join().unwrap();
                            }

                            //println!("Pedido completo: {:?}", order);
                            if self.is_leader(self.id) {
                                let commit_msg = MessageType::Commit(Commit { order_id: order.order_id });
                                let _ = self.tx_screen_sender_channel.send(commit_msg);
                                self.remove_completed_order(self.id, order.order_id);
                                let ordered_delivered_msg = MessageType::OrderDelivered(OrderDelivered {
                                    robot_id_maker: self.id,
                                    order_id: order.order_id,
                                });
                                let _ = tx_sender.send(ordered_delivered_msg);
                            } else {
                                let ordercomplete_msg = MessageType::OrderComplete(OrderComplete {
                                    robot_id_maker: self.id,
                                    order_id: order.order_id,
                                });
                                let _ = tx_sender.send(ordercomplete_msg);
                            }
                        }
                        _ => {
                            //eprintln!("Order Handler received non-prepare message");
                        }
                    }
                },
                recv(rx_token) -> token_message => {
                    if let Ok(MessageType::Token(token)) = token_message {
                        self.set_token_status(token.flavour.clone(), false);
                        self.update_tokens_table(&token.flavour.clone(), token.available_ammount.clone(), 0);
                        let _ = tx_sender.send(MessageType::Token(token));
                    }
                }
            }
        }
    }

    /// Simula el uso de un token para preparar helado.
    pub fn use_token(&self, mut token: Token, ammount: i32) -> Token {
        println!("Llenando pote de helado con {:?}", token.flavour);
        thread::sleep(Duration::from_millis(SLEEP_FACTOR * ammount as u64));
        token.available_ammount = token.available_ammount - ammount;
        println!("Termine de usar {:?}", token.flavour);
        return token;
    }

    /// Encuentra el próximo ID de robot al que se debe conectar.
    pub fn find_next_id(&self) -> Option<usize> {
        let network_lock = self.network.lock().unwrap();
        network_lock.get(&self.id).copied()
    }

    /// Establece un nuevo robot líder en la red.
    pub fn set_new_leader(&self, my_id: usize, new_leader_id: usize) {
        let mut id = self.leader_id.lock().unwrap();
        *id = new_leader_id;
        //println!("Nuevo lider en la red: {}", new_leader_id);

        if my_id == new_leader_id {
            let mut is_leader = self.is_leader.lock().unwrap();
            *is_leader = true;
        }
    }

    /// Verifica si un ID específico está conectado al robot actual.
    pub fn is_connected_to_me(&self, dead_robot_id: usize) -> bool {
        let network = self.network.lock().unwrap();
        let my_id = self.id;

        if let Some(existing_value) = network.get(&my_id) {
            return *existing_value == dead_robot_id;
        }

        false
    }

    /// Obtiene los pedidos de un robot específico si existen.
    pub fn get_robot_orders(&self, robot_id: usize) -> Option<OrdersList> {
        let orders_table = self.orders_table.lock().unwrap();
        return orders_table.get_robot_orders(robot_id);
    }

    /// Verifica si el tamaño de la red es exactamente 2.
    pub fn is_net_size_2(&self) -> bool {
        let net_size = self.network_size.lock().unwrap();
        return *net_size == 2;
    }

    /// Elimina un robot muerto de la tabla de pedidos.
    pub fn remove_dead_from_orders_table(&self, dead_id: usize) {
        self.orders_table.lock().unwrap().remove_robot(dead_id);
    }

    /// Conecta al siguiente robot en la red mediante TCP.
    pub fn connect_to_next_robot(&self) {
        let tx_sender_channel: Arc<Sender<MessageType>> = self.tx_sender_channel.clone();
        let next_robot_id = self.find_next_id();

        match next_robot_id {
            Some(next_id) => {
                let next_addr = format!("127.0.0.1:{}", 10000 + next_id);
                match TcpStream::connect(&next_addr) {
                    Ok(mut stream) => {
                        println!("Robot {}: Connected to next robot id: {}", self.id, next_id);

                        let robot_intro_msg = MessageType::RobotIntroduction(RobotIntroduction {
                            sender_id: self.id,
                        });

                        let serialized = serialize_message(&robot_intro_msg)
                            .expect("Failed to serialize message");

                        let _ = stream.write_all(&serialized);

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

                                            if let Err(_) = stream.write_all(&serialized) {
                                                // Falla enviar por el socket
                                                // mandamos por canal para que se vuelva a procesar (en el nuevo sender)
                                                let _ = tx_sender_channel.send(message);

                                                //         eprintln!(
                                                //     "Robot {}: Failed to write to stream ; err = {:?}",
                                                //     self.id, e
                                                // );
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
                        eprintln!("Next Robot is Offline: {}", e);
                    }
                }
            }
            None => {
                eprintln!("Error: Next robot not found.");
            }
        }
    }

    /// Verifica si el ID especificado es el líder de la red.
    pub fn is_leader(&self, id: usize) -> bool {
        let leader = self.leader_id.lock().unwrap();
        return *leader == id;
    }

    /// Encuentra el robot con menos pedidos pendientes para asignarle un nuevo pedido.
    pub fn find_target_robot(&self) -> Option<usize> {
        let orders_table = self.orders_table.lock().unwrap();
        return orders_table.robot_with_least_orders().clone();
    }

    /// Agrega un nuevo pedido a la tabla interna de pedidos.
    pub fn add_new_order(
        &self,
        robot_id: usize,
        order_id: usize,
        order_details: HashMap<IceCreamFlavor, i32>,
    ) {
        self.orders_table
            .lock()
            .unwrap()
            .add_order_for_robot(robot_id, order_id, order_details);
    }

    /// Elimina un pedido completado de la tabla.
    pub fn remove_completed_order(&self, robot_id: usize, order_id: usize) {
        self.orders_table
            .lock()
            .unwrap()
            .remove_order_for_robot(robot_id, order_id)
    }

    /// Encuentra el ID del robot del cual se estaba leyendo en la red.
    pub fn find_prev_robot(&self, self_id: usize) -> Option<usize> {
        let network_lock = self.network.lock().unwrap();
        let mut reading_from_id: Option<usize> = None;
        for (key, value) in &*network_lock {
            if value == &self_id {
                reading_from_id = Some(*key);
            }
        }

        return reading_from_id;
    }

    /// Envía un mensaje de elección de líder al identificador especificado.
    pub fn send_leader_election_message(&self, id: usize, dead_leader_id: usize) {
        let election_msg = MessageType::Election(Election {
            sender_id: id,
            current_candidate_id: id,
            dead_leader_id: dead_leader_id,
        });
        let _ = self.tx_sender_channel.send(election_msg);
    }

    /// Inicializa los tokens de helado y los envía a través del canal de envío.
    pub fn initialize_tokens(&self) {
        let my_id = self.id;
        let now = SystemTime::now();

        let chocolate_token = MessageType::Token(Token {
            sender_id: my_id,
            flavour: IceCreamFlavor::Chocolate,
            last_modified_by_id: my_id,
            last_modification_timestamp: now,
            available_ammount: INITIAL_GRAMS_AMOUNT,
        });

        let mint_token = MessageType::Token(Token {
            sender_id: my_id,
            flavour: IceCreamFlavor::Mint,
            last_modified_by_id: my_id,
            last_modification_timestamp: now,
            available_ammount: INITIAL_GRAMS_AMOUNT,
        });

        let vanilla_token = MessageType::Token(Token {
            sender_id: my_id,
            flavour: IceCreamFlavor::Vanilla,
            last_modified_by_id: my_id,
            last_modification_timestamp: now,
            available_ammount: INITIAL_GRAMS_AMOUNT,
        });

        let strawberry_token = MessageType::Token(Token {
            sender_id: my_id,
            flavour: IceCreamFlavor::Strawberry,
            last_modified_by_id: my_id,
            last_modification_timestamp: now,
            available_ammount: INITIAL_GRAMS_AMOUNT,
        });

        let _ = self.tx_sender_channel.send(chocolate_token);
        let _ = self.tx_sender_channel.send(vanilla_token);
        let _ = self.tx_sender_channel.send(strawberry_token);
        let _ = self.tx_sender_channel.send(mint_token);
    }

    /// Actualiza la red después de que un robot específico muere.
    pub fn update_network(robot: Arc<Self>, dead_id: usize) {
        let mut net_size = robot.network_size.lock().unwrap();
        *net_size -= 1;

        let mut network = robot.network.lock().unwrap();

        let mut _id_connected_to_dead: Option<usize> = None;

        let mut dead_connected_to_id: Option<usize> = None;

        for (key, value) in &*network {
            if value == &dead_id {
                dead_connected_to_id = Some(*key);
                break;
            }
        }

        _id_connected_to_dead = network.get(&dead_id).copied();

        network.remove(&dead_id);

        if let (Some(dead_id), Some(new_id)) = (dead_connected_to_id, _id_connected_to_dead) {
            *network.get_mut(&dead_id).unwrap() = new_id;
        }

        //println!("Updated network: {:?}", network); // debug print
    }
}
