-edge(a,x). -edge(a,y). edge(b,z).
allowed(x). allowed(y). allowed(z).
{p(a); p(b)}.
:- -edge(X,Y), allowed(Y), p(X).
