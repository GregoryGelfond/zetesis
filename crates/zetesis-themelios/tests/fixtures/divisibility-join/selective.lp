row(1..24,ordinary).
row(6,chosen).
divisor(3,known).
divisible(X) :- row(X,chosen), divisor(D,known), X\D=0.
