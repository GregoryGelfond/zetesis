num(1..24).
divisor(3).
divisible(X) :- num(X), divisor(D), X\D=0.
#show divisible/1.
