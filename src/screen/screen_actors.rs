extern crate actix;
use actix::AsyncContext;
// use crate::actix::AsyncContext;
// use crate::Addr;
use actix::{Actor, Context, Handler, Message, Recipient};
// use heladeria::common::flavors::{default_flavors, IceCreamFlavor};
// use heladeria::common::messages::*;
use crate::common::constants::{FIRST_BATCH, MULTIPLICATION_BATCH};
use crate::common::flavors::default_flavors;
use crate::common::flavors::IceCreamFlavor;
use crate::common::messages::{MessageType, OrderScreen};
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::str::FromStr;
use std::sync::mpsc::Sender;
use std::sync::Arc;
const PREPARE: &'static str = "PREPARE";
const COMMIT: &'static str = "COMMIT";
const ABORT: &'static str = "ABORT";
use actix::Addr;

/// Representa un actor ScreenActor responsable del procesamiento de pedidos.
pub struct ScreenActor {
    valid_flavors: HashSet<IceCreamFlavor>,
    processor: Recipient<PrepareOrder>,
    orders: Orders,
}

impl ScreenActor {
    /// Crea una nueva instancia de ScreenActor.
    pub fn new(processor: Recipient<PrepareOrder>) -> Self {
        let valid_flavors = default_flavors();
        ScreenActor {
            valid_flavors,
            processor,
            orders: Orders { orders: Vec::new() },
        }
    }
    /// Verifica si un sabor es válido.
    fn is_valid_flavor(&self, flavor: &IceCreamFlavor) -> bool {
        self.valid_flavors.contains(flavor)
    }
    /// Toma y procesa un pedido.
    fn take_and_process_order(&mut self) {
        if let Some(order) = self.orders.orders.pop() {
            if self.validate_flavours(order.clone()) {
                let _ = self.processor.do_send(PrepareOrder { order: order });
            } else {
                self.take_and_process_order()
            }
        }
    }
    /// Valida los sabores de un pedido.
    fn validate_flavours(&mut self, order: Order) -> bool {
        let mut invalid_flavors = Vec::new(); // Lista para guardar sabores inválidos

        let is_valid_order = order.flavors.iter().all(|flavor| {
            match IceCreamFlavor::from_str(&flavor.name) {
                Ok(valid_flavor) => {
                    let is_valid = self.is_valid_flavor(&valid_flavor);
                    if !is_valid {
                        invalid_flavors.push(flavor.name.clone()); // Agregar sabor inválido a la lista
                    }
                    is_valid
                }
                Err(_) => {
                    invalid_flavors.push(flavor.name.clone()); // Agregar sabor inválido a la lista si no se puede convertir
                    false
                }
            }
        });
        if !is_valid_order {
            println!("Pedido DESCARTADO ya que tiene estos sabores no existentes:");
            for flavor in invalid_flavors {
                println!("- {}", flavor);
            }
        }
        is_valid_order
    }
}

/// Implementación del actor para ScreenActor.
impl Actor for ScreenActor {
    type Context = Context<Self>;
}

/// Representa un actor FileReaderActor responsable de leer archivos de pedidos.
pub struct FileReaderActor {
    file_reader: Recipient<Orders>,
}

impl FileReaderActor {
    /// Crea una nueva instancia de FileReaderActor.
    pub fn new(file_reader: Recipient<Orders>) -> Self {
        FileReaderActor { file_reader }
    }
}
/// Implementación del actor para FileReaderActor.
impl Actor for FileReaderActor {
    type Context = Context<Self>;
}

/// Representa un actor ProcessorActor responsable de gestionar pedidos autorizados.
pub struct ProcessorActor {
    id: usize,
    gateway: TcpStream,
    sender_channel: Arc<Sender<MessageType>>,
    screen_address: Option<Addr<ScreenActor>>,
}

impl Actor for ProcessorActor {
    type Context = Context<Self>;
}

impl ProcessorActor {
    /// Crea una nueva instancia de ProcessorActor.
    pub fn new(
        id: usize,
        gateway_addr: String,
        sender_channel: Arc<Sender<MessageType>>,
    ) -> Result<Self, String> {
        match TcpStream::connect(gateway_addr) {
            Ok(gateway) => Ok(ProcessorActor {
                id,
                gateway,
                sender_channel,
                screen_address: None,
            }),
            Err(e) => Err(format!("Failed to connect to local server: {}", e)),
        }
    }

    /// Escribe datos en el socket TCP.
    pub fn write(&mut self, buf: Vec<u8>) -> Result<(), String> {
        let _ = self.gateway.write_all(&buf).map_err(|e| e.to_string());
        Ok(())
    }
    /// Lee datos del socket TCP.
    pub fn read(&mut self, buffer_size: usize) -> Result<String, String> {
        let mut buffer = vec![0; buffer_size];
        let bytes_read = self.gateway.read(&mut buffer).map_err(|e| e.to_string())?;
        let response = String::from_utf8_lossy(&buffer[..bytes_read])
            .trim()
            .to_string();
        Ok(response)
    }
    /// Realiza un commit para un pedido autorizado.
    fn commit(&mut self, order_id: u32) -> Result<(), String> {
        let reply = format!("{},{},", COMMIT, order_id);
        self.write(reply.into_bytes()).unwrap();
        Ok(())
    }
    /// Aborta un pedido no autorizado.
    fn abort(&mut self, order_id: u32) -> Result<(), String> {
        let reply = format!("{},{},", ABORT, order_id);
        self.write(reply.into_bytes()).unwrap();
        Ok(())
    }
    /// Imprime los detalles de un pedido autorizado.
    fn print_authorized(&mut self, order: Order, order_id: u32) {
        println!("Pedido AUTORIZADO con ID {}:", order_id);
        for flavor in order.flavors {
            println!("  - {}: {}", flavor.name, flavor.grams);
        }
    }
    /// Imprime los detalles de un pedido rechazado.
    fn print_rejected(&mut self, order: Order) {
        println!("Pedido RECHAZADO:");

        for flavor in order.flavors {
            println!("  - {}: {}", flavor.name, flavor.grams);
        }
    }
    /// Autoriza un pedido con el servidor remoto.
    fn authorize(&mut self, order: &str) -> Result<u32, String> {
        let prepare_msg = format!("{},{}", PREPARE, order);
        let _ = self.write(prepare_msg.into_bytes());
        let response = self.read(1024)?;
        //println!(" Received: {}", response);
        // Handle response
        if response.starts_with(ABORT) {
            return Ok(0);
        }
        if response.starts_with(COMMIT) {
            let parts: Vec<&str> = response.split(',').collect();
            if parts.len() >= 2 {
                let order_id = match parts[1].parse::<u32>() {
                    Ok(id) => Ok(id),
                    Err(_) => {
                        eprintln!("Invalid order ID in COMMIT message");
                        Err("Invalid order ID".to_string())
                    }
                };
                return order_id;
            }
        }
        Err("Unexpected response format".to_string())
    }
    /// Envía un pedido autorizado al ScreenActor.
    fn send_authorized_order(&mut self, order: Order, order_id: u32) {
        let mut order_details = HashMap::new();
        for flavor in order.flavors.iter() {
            if let Ok(valid_flavor) = IceCreamFlavor::from_str(&flavor.name) {
                *order_details.entry(valid_flavor).or_insert(0) += flavor.grams;
            }
        }
        let order_msg = MessageType::OrderScreen(OrderScreen {
            sender_id: self.id,
            order_id: order_id as usize,
            order_details,
        });

        // Enviar el mensaje Prepare
        if let Err(e) = self.sender_channel.send(order_msg) {
            eprintln!("Error al enviar el mensaje Order: {:?}", e);
        }
    }
}

/*
---------------------------------------- READ FILE ---------------------------------------------
*/

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReadFile {
    name: String,
}

impl ReadFile {
    pub fn new(name: String) -> Self {
        ReadFile { name }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct Flavor {
    name: String,
    grams: i32,
}

#[derive(Debug, Deserialize, Message, Clone)]
#[rtype(result = "()")]
pub struct Order {
    flavors: Vec<Flavor>,
    //total_grams: u32,
}

impl Order {
    fn to_string(&self) -> String {
        let mut order_string = String::new();

        for flavor in &self.flavors {
            order_string.push_str(&format!("{},{}g;", flavor.name, flavor.grams));
        }

        order_string
    }
}

#[derive(Debug, Deserialize, Message, Clone)]
#[rtype(result = "()")]
pub struct Orders {
    orders: Vec<Order>,
}

impl Handler<ReadFile> for FileReaderActor {
    //type Result = ResponseActFuture<Self, ()>;
    type Result = ();
    fn handle(&mut self, msg: ReadFile, _ctx: &mut Context<Self>) -> () {
        println!("[FileReader] Me mandaron a leer este archivo: {}", msg.name);

        let file = File::open(msg.name).expect("Unable to open file");
        let reader = BufReader::new(file);
        let orders: Orders = serde_json::from_reader(reader).expect("Unable to parse JSON");
        let _ = self.file_reader.do_send(orders);
    }
}

/*
------------------------------------------ READ ORDERS ---------------------------------------------
*/

impl Handler<Orders> for ScreenActor {
    type Result = ();

    fn handle(&mut self, msg: Orders, ctx: &mut Context<Self>) {
        self.orders.orders.extend(msg.orders.clone());
        // Crear un nuevo mensaje SendOrder con el número de pedidos a enviar
        let send_order_msg = SendOrder {
            send_new_orders: FIRST_BATCH,
        };

        // Enviar el mensaje SendOrder
        ctx.address().do_send(send_order_msg);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendOrder {
    send_new_orders: usize,
}

/// El FileReaderActor enviará el detalle de todos los pedidos leídos al ScreenActor
/// ScreenActor le manda lo leido del archivo a processor para autorizarlo
impl Handler<SendOrder> for ScreenActor {
    type Result = ();

    fn handle(&mut self, msg: SendOrder, _ctx: &mut Context<Self>) {
        for _ in 0..msg.send_new_orders {
            self.take_and_process_order();
        }
    }
}

/*
------------------------------------------ PREPARE ORDERS ---------------------------------------------
*/

#[derive(Message)]
#[rtype(result = "()")]
pub struct PrepareOrder {
    order: Order,
}

/// ScreenActor enviará los pedidos al ProcessorActor
/// Este los autorizará con el gateway y lo mandará al anillo
impl Handler<PrepareOrder> for ProcessorActor {
    type Result = ();

    fn handle(&mut self, msg: PrepareOrder, _ctx: &mut Context<Self>) -> () {
        let order_str = msg.order.to_string();
        match self.authorize(&order_str) {
            Ok(order_id) => {
                if order_id > 0 {
                    //Envio pedido al anillo
                    self.send_authorized_order(msg.order.clone(), order_id);
                    self.print_authorized(msg.order, order_id);
                } else {
                    if let Some(ref screen_address) = self.screen_address {
                        screen_address.do_send(SendOrder { send_new_orders: 1 });
                    } //Autorizo otro pedido porque este fue rechazado
                    self.print_rejected(msg.order);
                }
            }
            Err(e) => eprintln!("Error al autorizar pedido: {}", e),
        }
    }
}

/*
------------------------------------------ COMMIT/ABORT ORDER---------------------------------------------
*/

#[derive(Message)]
#[rtype(result = "()")]
pub struct CommitOrder {
    pub order_id: u32,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AbortOrder {
    pub order_id: u32,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Address {
    pub screen_addr: Addr<ScreenActor>,
}

/// ScreenActor lider enviará los pedidos realizados/cancelados al ProcessorActor
/// Este los registrará en el gateway
impl Handler<CommitOrder> for ProcessorActor {
    type Result = ();

    fn handle(&mut self, msg: CommitOrder, _ctx: &mut Context<Self>) -> () {
        //let screen_addr = msg.screen_addr;
        let _ = self.commit(msg.order_id);
        if let Some(ref screen_address) = self.screen_address {
            screen_address.do_send(SendOrder {
                send_new_orders: MULTIPLICATION_BATCH,
            }); // Aquí asumimos que queremos enviar un pedido nuevo
        }
    }
}

impl Handler<AbortOrder> for ProcessorActor {
    type Result = ();

    fn handle(&mut self, msg: AbortOrder, _ctx: &mut Context<Self>) -> () {
        //let screen_addr = msg.screen_addr;
        let _ = self.abort(msg.order_id);
        if let Some(ref screen_address) = self.screen_address {
            screen_address.do_send(SendOrder {
                send_new_orders: MULTIPLICATION_BATCH,
            }); // Aquí asumimos que queremos enviar un pedido nuevo
        }
    }
}

impl Handler<Address> for ProcessorActor {
    type Result = ();

    fn handle(&mut self, msg: Address, _ctx: &mut Context<Self>) -> () {
        //let screen_addr = msg.screen_addr;
        self.screen_address = Some(msg.screen_addr);
    }
}
