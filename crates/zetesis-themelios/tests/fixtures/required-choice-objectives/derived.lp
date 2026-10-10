{enabled; gate}.
1 { choose(a); choose(b) } 1 :- enabled, gate.
occurs(X) :- choose(X), enabled.
#minimize { 2@2,a:occurs(a); 2@2,b:occurs(b); 1@1,t:gate }.
