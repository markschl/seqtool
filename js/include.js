'use strict';

function Int(x) {
    let i = parseInt(x);
    if (isNaN(i)) {
        throw `Could not convert '${x}' to integer`;
    }
    if (x.includes(".")) {
        throw `Could not convert decimal number '${x}' to integer`;
    }
    return i;
}

function Num(x) {
    let f = parseFloat(x);
    if (isNaN(f)) {
        throw `Could not convert '${x}' to decimal number`;
    }
    return f;
}
