//! Original programs shared by source execution and physical qualification.

pub const SOURCES: [&str; 10] = [
    "1#min{1:a;1:b}1.",
    "1#max{0:a;1:a}1.",
    "1#min{1:#true;1:a}1.",
    "a.2{#true;#true}2.",
    "d(1..2).1{#true:d(X)}1.",
    "{a;b}.1{#true:a;#true:b}1.",
    "1#count{0:#true;0:a}1.",
    "0{#false;a}0.",
    "a:-a.1{#true:a}1.",
    "0#sum+{0:#true;0:a}0.",
];
