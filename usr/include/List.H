#ifndef LIST_H
#define LIST_H

#include "BTConfig.H"

#include <assert.h>

#include "AbsList.H"
#include "AbsListElement.H"
#include "ListElement.H"

template<class T>
class List : private AbsList {

// ListIter needs to be a friend class to make implicit conversion
// from ListElement<T> to AbsListElement.
friend class ListIter<T>;
friend class RWListIter<T>; 

private:
  List (const List<T>& other) {};
  List<T>& operator= (const List<T>& other) { return *this; };
  int num_iters_;
  RWListIter<T> *rw_iter_;
  inline void add_iter_ (ListIter<T> *new_iter);
  inline void add_rw_iter_ (RWListIter<T> *new_iter);
  inline void remove_iter_ (ListIter<T> *old_iter);
  inline void remove_rw_iter_ (RWListIter<T> *old_iter);
  int iters_() { return num_iters_; }
  inline ListElement<T> *alloc_elem (const T &data);
  inline void dealloc_elem (ListElement<T> *data);

public:
  inline List();
  inline ~List();
  inline void insert_before_head (const T& data);
  inline void insert_after_tail (const T& data);
  inline int remove_head (T& data);
  inline int remove_tail (T& data);
  inline int empty();
};

#include "ListIter.H"
#include "RWListIter.H"

template<class T> List<T>::List() 
: AbsList(), num_iters_ (0), rw_iter_ (0) {} 

template<class T> ListElement<T> *List<T>::alloc_elem (const T &data) {
  return new ListElement<T> (data);
}

template<class T> void List<T>::dealloc_elem (ListElement<T> *data) {
  if (!data)
    return;
  delete data;
}

template<class T> List<T>::~List() {
  T data;
  while (remove_head (data));
}

template<class T> void List<T>::add_rw_iter_ (RWListIter<T> *new_iter) {
  // Multiple rw iters aren't supported.
  assert(rw_iter_ == (RWListIter<T> *) 0);
  rw_iter_ = new_iter;
}

template<class T> void List<T>::add_iter_ (ListIter<T> *new_iter) {
  num_iters_++;
}

template<class T> void List<T>::remove_iter_ (ListIter<T> *new_iter) {
  num_iters_--;
}

template<class T> void List<T>::remove_rw_iter_ (RWListIter<T> *old_iter) {
  rw_iter_ = 0;
}

template<class T> void List<T>::insert_before_head (const T& data) {
  ListElement<T> *new_elem;
  new_elem = alloc_elem (data);
  AbsList::insert_before_head (new_elem);
}

template<class T> void List<T>::insert_after_tail(const T& data) {
  ListElement<T> *new_elem;
  new_elem = alloc_elem (data);
  AbsList::insert_after_tail (new_elem);
}

template<class T> int List<T>::remove_head (T& data) {
  ListElement<T> *old_elem;
  if (AbsList::empty())
    return 0;
  old_elem = (ListElement<T> *) AbsList::remove_head();
  data = old_elem->data();
  dealloc_elem (old_elem);
  return 1;
}

template<class T> int List<T>::remove_tail (T& data) {
  ListElement<T> *old_elem;
  if (AbsList::empty())
    return 0;
  old_elem = (ListElement<T> *) AbsList::remove_tail();
  data = old_elem->data();
  dealloc_elem (old_elem);
  return 1;
}

template<class T> int List<T>::empty() {
  return AbsList::empty();
}

#endif
