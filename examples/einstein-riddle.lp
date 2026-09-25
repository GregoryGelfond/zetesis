% Einstein's Riddle
% Gregory Gelfond
%
% 1.  There are five houses.
% 2.  The Englishman lives in the red house.
% 3.  The Spaniard owns the dog.
% 4.  Coffee is drunk in the green house.
% 5.  The Ukrainian drinks tea.
% 6.  The green house is immediately to the right of the ivory house.
% 7.  The Old Gold smoker owns snails.
% 8.  Kools are smoked in the yellow house.
% 9.  Milk is drunk in the middle house.
% 10. The Norwegian lives in the first house.
% 11. The man who smokes Chesterfields lives in the house next to the man
%     with the fox.
% 12. Kools are smoked in the house next to the house where the horse is
%     kept.
% 13. The Lucky Strike smoker drinks orange juice.
% 14. The Japanese smokes Parliaments.
% 15. The Norwegian lives next to the blue house.
%
% Now, who drinks water? Who owns the zebra?

% There are five houses, numbered from 1..5.
house(1..5).

% The people are: "The Englishman", "The Spaniard", "The Ukrainian", "The
% Norwegian", and "The Japanese".
person(
   "The Englishman";
   "The Spaniard";
   "The Ukrainian";
   "The Norwegian";
   "The Japanese"
).

% The beverages are: "Coffee", "Tea", "Milk", "Water", and "Orange Juice".
beverage(
   "Coffee";
   "Tea";
   "Milk";
   "Water";
   "Orange Juice"
).

% The pets are: "Dog", "Horse", "Snail", "Zebra", and "Fox".
pet(
   "Dog";
   "Horse";
   "Snail";
   "Zebra";
   "Fox"
).

% The cigarette brands are: "Chesterfields", "Lucky Strikes", "Kools",
% "Old Gold", and "Parliaments".
brand(
   "Chesterfields";
   "Lucky Strikes";
   "Kools";
   "Old Gold";
   "Parliaments"
).

% The colors are: "Red", "Green", "Blue", "Yellow", and "Ivory".
color(
   "Red";
   "Green";
   "Blue";
   "Yellow";
   "Ivory"
).

% Every person lives in exactly one house.
1{ lives_in(P,H) : house(H) }1 :- person(P).

% A house cannot have more than one person living in it.
:- house(H), #count{ P : lives_in(P,H) } > 1.

% Every person drinks exactly one beverage.
1{ drinks(P,B) : beverage(B) }1 :- person(P).

% A beverage cannot be drunk by more than one person.
:- beverage(B), #count{ P : drinks(P,B) } > 1.

% Every person owns exactly one pet.
1{ owns(P,X) : pet(X) }1 :- person(P).

% A pet cannot be owned by more than one person.
:- pet(P), #count{ X : owns(X,P) } > 1.

% Every persons smokes exactly one brand of cigarette.
1{ smokes(P,B) : brand(B) }1 :- person(P).

% A cigarette brand cannot be smoked by more than one person.
:- brand(B), #count{ P : smokes(P,B) } > 1.

% Every house is painted exactly one color.
1{ painted(H,C) : color(C) }1 :- house(H).

% A color cannot be used to paint more than one house.
:- color(C), #count{ H : painted(H,C) } > 1.

% The Englishman lives in the red house.
:- lives_in("The Englishman", H),
  not painted(H, "Red").

% The Spaniard owns the dog.
:- not owns("The Spaniard", "Dog").

% Coffee is drunk in the green house.
:- not drunk_in_house_colored("Coffee", "Green").

% The Ukrainian drinks tea.
:- not drinks("The Ukrainian", "Tea").

% The green house is immediately to the right of the ivory house.
:- painted(H1, "Green"),
  painted(H2, "Ivory"),
  not immediately_right_of(H1, H2).

% The Old Gold smoker owns snails.
:- smokes(P, "Old Gold"),
  not owns(P, "Snail").

% Kools are smoked in the yellow house.
:- not smoked_in_house_colored("Kools", "Yellow").

% Milk is drunk in the middle house.
:- not drunk_in_middle_house("Milk").

% The Norwegian lives in the first house.
:- not lives_in_first_house("The Norwegian").

% The man who smokes Chesterfields lives in the house next to the man
% with the fox.
:- smokes(P, "Chesterfields"), not neighbors_with_pet_owner(P, "Fox").

% The man who smokes Kools is lives next to the man who owns the horse.
:- smokes(P, "Kools"), not neighbors_with_pet_owner(P, "Horse").

% The Lucky Strike smoker drinks orange juice.
:- smokes(P, "Lucky Strikes"), not drinks(P, "Orange Juice").

% The Japanese smokes Parliaments.
:- not smokes("The Japanese", "Parliaments").

% The Norwegian lives next to the blue house.
:- not neighbors_house_painted("The Norwegian", "Blue").

%*
* Auxiliary Relations
*%

% We say that a beverage is drunk in the house colored C if the beverage
% is drunk by the person who lives in the house painted C.
drunk_in_house_colored(B, C) :-
   person(P),
   drinks(P, B),
   lives_in(P, H),
   painted(H, C).

% We say that a house H1 is immediately to the right of a house H2 if
% H1 = H2 + 1.
immediately_right_of(H1, H2) :-
   house(H1),
   house(H2),
   H1 = H2 + 1.

% We say that a house H1 is immediately to the left of a house H2 if
% H1 = H2 - 1.
immediately_left_of(H1, H2) :-
   house(H1),
   house(H2),
   H1 = H2 - 1.

% We say that person P1 is neighbors with a person P2 if P1 lives
% immediately to the left or right of P2.
neighbors_with(P1, P2) :-
   person(P1),
   person(P2),
   lives_in(P1, H1),
   lives_in(P2, H2),
   immediately_left_of(H1, H2).

neighbors_with(P1, P2) :-
   person(P1),
   person(P2),
   lives_in(P1, H1),
   lives_in(P2, H2),
   immediately_right_of(H1, H2).

% We say that a cigarette brand is smoked in the house colored C if the
% cigarette brand is smoked by the person who lives in the house painted C.
smoked_in_house_colored(B, C) :-
   person(P),
   smokes(P, B),
   lives_in(P, H),
   painted(H, C).

% We say that a beverage is drunk in the middle house if the beverage is
% drunk by the person who lives in the middle house (which is house #3).
drunk_in_middle_house(B) :-
   beverage(B),
   drinks(P, B),
   lives_in(P, 3).

% We say that a person lives in the first house if the person is
% living in house #1.
lives_in_first_house(P) :-
   person(P),
   lives_in(P, 1).

% We say that a person P is neighbors with the owner of the pet X if
% the person is neighbors with the person who owns X.
neighbors_with_pet_owner(P, X) :-
   person(P),
   person(O),
   pet(X),
   owns(O, X),
   neighbors_with(P, O).

% We say that a person P1 neighbors the house painted C if P1 is
% neighbors with person P2 and P2 lives in the house painted C.
neighbors_house_painted(P1, C) :-
   neighbors_with(P1, P2),
   lives_in(P2, H),
   painted(H, C).

%*
* Exported/Output Relations
*%

solution(House, Color, Person, Pet, Beverage, Brand) :-
   lives_in(Person, House),
   painted(House, Color),
   drinks(Person, Beverage),
   owns(Person, Pet),
   smokes(Person, Brand).

#show solution/6.
