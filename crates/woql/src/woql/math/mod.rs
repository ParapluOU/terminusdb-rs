mod val;
mod plus;
mod min;
mod times;
mod div;
mod exp;
mod floor;
mod less;
mod gt;
mod sum;
mod eq;
mod eval;

use crate::*;

pub use val::*;
pub use plus::*;
pub use min::*;
pub use times::*;
pub use div::*;
pub use exp::*;
pub use floor::*;
pub use less::*;
pub use gt::*;
pub use sum::*;
pub use eq::*;
pub use eval::*;

ast_struct!(
    ArithmeticExpression {
        value(Box<ArithmeticValue>),
        plus(Box<Plus>),
        minus(Box<Minus>),
        times(Box<Times>),
        divide(Box<Divide>),
        div(Box<Div>),
        exp(Box<Exp>),
        floor(Box<Floor>),
    }
);

// #[derive(Clone, Debug, TerminusDBSchema, FromVariants)]
// pub enum ArithmeticExpression {
//     value(Box<ArithmeticValue>),
//     plus(Box<Plus>),
//     minus(Box<Minus>),
//     times(Box<Times>),
//     divide(Box<Divide>),
//     div(Box<Div>),
//     exp(Box<Exp>),
//     floor(Box<Floor>),
// }