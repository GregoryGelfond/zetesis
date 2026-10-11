num(2147483638..2147483647).
divisor(2147483647).
divisible(X) :- num(X), divisor(D), X\D=0.
