#ifndef LISTELEMENT_H
#define LISTELEMENT_H

#include "BTConfig.H"
#include "AbsListElement.H"

template<class T> class List;
template<class T> class ListIter;
template<class T> class RWListIter;

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
