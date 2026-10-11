num(1..24).
divisor(3).
divisible(X) :- num(X), divisor(D), D>0, X\D=0, X<10.
