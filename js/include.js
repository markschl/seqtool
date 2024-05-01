'use strict';

/* Converts a string to an integer, or throws an error. Leaves null/undefined as is */
function int(x) {
    let i = parseInt(x);
    if (isNaN(i)) {
        if (x === undefined) return undefined;
        if (x === null) return null;
        throw `Could not convert '${x}' to integer`;
    }
    if (x.includes(".")) {
        throw `Could not convert decimal number '${x}' to integer`;
    }
    return i;
}

/* Converts a string to a float, or throws an error. Leaves null/undefined as is */
function num(x) {
    let f = parseFloat(x);
    if (isNaN(f)) {
        if (x === undefined) return undefined;
        if (x === null) return null;
        throw `Could not convert '${x}' to decimal number`;
    }
    return f;
}
