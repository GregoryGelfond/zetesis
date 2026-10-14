num(1;2147483647).
divisor(2).
divisible(X) :- num(X), divisor(D), X\D=0.
