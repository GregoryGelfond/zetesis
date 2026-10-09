edge(a,x). edge(a,y). edge(b,z).
allowed(x). allowed(y). allowed(z).
{p(a); q(a); p(b); q(b)}.
:- edge(X,Y), allowed(Y), p(X), q(X).
