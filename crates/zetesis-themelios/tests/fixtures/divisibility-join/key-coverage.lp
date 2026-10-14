num(1..24).
divisor(3).
optional(1).
divisible(X) :- num(X), divisor(D), optional(Y), not absent(Y), X\D=0.
