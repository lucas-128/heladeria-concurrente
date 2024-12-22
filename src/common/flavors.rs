/// Módulo para manejar sabores de helado y su información relacionada.
///
/// Este módulo define una enumeración para diferentes sabores de helado, así como funciones y estructuras
/// para manejar información de inventario y conversiones entre cadenas de caracteres y los sabores de helado.
use serde::{Deserialize, Serialize};
use std::fmt;
use std::{collections::HashSet, str::FromStr, time::SystemTime};

/// Enumeración de los diferentes sabores de helado disponibles.
#[derive(Serialize, Deserialize, Debug, Hash, Eq, PartialEq, Clone)]
pub enum IceCreamFlavor {
    /// Sabor Vainilla.
    Vanilla,
    /// Sabor Chocolate.
    Chocolate,
    /// Sabor Frutilla.
    Strawberry,
    /// Sabor Menta.
    Mint,
}
impl IceCreamFlavor {
    /// Devuelve un iterador sobre los sabores de helado disponibles.
    pub fn iter() -> std::slice::Iter<'static, IceCreamFlavor> {
        static FLAVORS: [IceCreamFlavor; 4] = [
            IceCreamFlavor::Vanilla,
            IceCreamFlavor::Chocolate,
            IceCreamFlavor::Strawberry,
            IceCreamFlavor::Mint,
        ];
        FLAVORS.iter()
    }
}

/// Estructura que contiene información sobre un sabor de helado específico.
#[derive(Debug, Clone)]

pub struct FlavorInfo {
    pub has_token: bool,
    pub stock: i32,
    pub last_modification_timestamp: SystemTime,
}

/// Devuelve un conjunto con los sabores de helado por defecto.
pub fn default_flavors() -> HashSet<IceCreamFlavor> {
    [
        IceCreamFlavor::Vanilla,
        IceCreamFlavor::Chocolate,
        IceCreamFlavor::Strawberry,
        IceCreamFlavor::Mint,
    ]
    .iter()
    .cloned()
    .collect()
}

/// Convierte una cadena de caracteres en un sabor de helado.
impl FromStr for IceCreamFlavor {
    type Err = ();
    fn from_str(input: &str) -> Result<IceCreamFlavor, Self::Err> {
        match input {
            "Vanilla" => Ok(IceCreamFlavor::Vanilla),
            "Chocolate" => Ok(IceCreamFlavor::Chocolate),
            "Strawberry" => Ok(IceCreamFlavor::Strawberry),
            "Mint" => Ok(IceCreamFlavor::Mint),
            _ => Err(()),
        }
    }
}

/// Formatea el sabor de helado como una cadena de caracteres.
impl fmt::Display for IceCreamFlavor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IceCreamFlavor::Vanilla => write!(f, "Vanilla"),
            IceCreamFlavor::Chocolate => write!(f, "Chocolate"),
            IceCreamFlavor::Strawberry => write!(f, "Strawberry"),
            IceCreamFlavor::Mint => write!(f, "Mint"),
        }
    }
}
