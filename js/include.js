'use strict';

function Int(x) {
    let i = parseInt(x);
    if (isNaN(i)) {
        throw `Not an integer: ${x}`;
    }
    if (x.includes(".")) {
        throw `Decimal number cannot be converted to integer: ${x}`;
    }
    return i;
}

function Num(x) {
    let f = parseFloat(x);
    if (isNaN(f)) {
        throw `Not a number: ${x}`;
    }
    return f;
}
