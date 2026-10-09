edge(a,x,x). edge(a,y,y). edge(b,z,w).
allowed(x). allowed(y). allowed(z). allowed(w).
{p(a); p(b)}.
:- edge(X,Y,Y), allowed(Y), p(X).
