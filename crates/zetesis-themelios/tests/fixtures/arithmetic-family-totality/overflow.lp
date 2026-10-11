num(1;2147483647).
divisor(1).
:- num(X), divisor(D), X/D+1>0.
