edge(a,x). edge(a,y). edge(b,z).
allowed(x). allowed(y). allowed(z).
{p(a); q(a); p(b); q(b)}.
:- p(a), q(a).
:- p(b), q(b).
