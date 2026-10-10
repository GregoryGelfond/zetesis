1 { a; b } 1.
p :- a.
q :- p.
p :- q.
q :- b.
#minimize { 1,p:p }.
