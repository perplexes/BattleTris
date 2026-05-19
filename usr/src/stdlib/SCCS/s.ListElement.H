h58424
s 00000/00000/00000
d R 1.2 01/10/20 13:35:04 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/stdlib/ListElement.H
c Name history : 1 0 src/stdlib/ListElement.H
e
s 00023/00000/00000
d D 1.1 01/10/20 13:35:03 bmc 1 0
c date and time created 01/10/20 13:35:03 by bmc
e
u
U
f e 0
t
T
I 1
#ifndef LISTELEMENT_H
#define LISTELEMENT_H

#include "BTConfig.H"
#include "AbsListElement.H"

template<class T>
class ListElement : private AbsListElement {
  
friend class List<T>;
friend class ListIter<T>;
friend class RWListIter<T>;

private:
  const T data_;

public: 
  ListElement (const T& data) : AbsListElement (0,0), data_ (data) {}
  ~ListElement() {}
  const T& data() { return data_; }
};

#endif
E 1
