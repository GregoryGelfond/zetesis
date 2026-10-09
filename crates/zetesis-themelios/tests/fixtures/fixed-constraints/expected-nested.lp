edge(a,x). edge(a,y). edge(b,z).
allowed(x). allowed(y). allowed(z).
{p(f(a)); q(f(a)); p(f(b)); q(f(b))}.
:- p(f(a)), q(f(a)).
:- p(f(b)), q(f(b)).
