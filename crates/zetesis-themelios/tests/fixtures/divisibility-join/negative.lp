num(-6;-3;0;3;6).
divisor(3).
divisible(X) :- num(X), divisor(D), X\D=0.
