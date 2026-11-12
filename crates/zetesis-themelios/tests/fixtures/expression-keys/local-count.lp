% Repeated inherited bounds, irrelevant outer values, and non-key source order.
b(0;2;4).
i(a;b).
{p(3;1;2)}.
out(I,B,N) :- i(I), b(B), N=#count{X:p(X),X<B}.
