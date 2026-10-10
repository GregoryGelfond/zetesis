1 { a; b } 1.
{c}.
p :- q.
q :- p.
p :- c.
#minimize { 1,p:p }.
