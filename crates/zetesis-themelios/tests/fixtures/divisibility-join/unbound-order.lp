number(2;3).
divisor(2..12).
divisible(X) :- number(X), divisor(D), X\D=0.
