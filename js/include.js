'use strict';

function num(x) {
    let f = parseFloat(x);
    if (isNaN(f)) {
        if (x === undefined) return undefined;
        if (x === null) return null;
        throw `Could not convert '${x}' to a decimal number`;
    }
    return f;
}
