% Independent witnesses share a body after its arithmetic head values are fixed.
% The constraint keeps h in the retained program under either grounding mode.
{ p(1,a); p(1,b); q(2,a); q(2,b) }.
h(X,X+1) :- p(X,_), q(X+1,_).
:- not h(1,2).
