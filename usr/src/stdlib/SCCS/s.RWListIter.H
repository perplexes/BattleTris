h01822
s 00000/00000/00000
d R 1.2 01/10/20 13:35:05 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/stdlib/RWListIter.H
c Name history : 1 0 src/stdlib/RWListIter.H
e
s 00084/00000/00000
d D 1.1 01/10/20 13:35:04 bmc 1 0
c date and time created 01/10/20 13:35:04 by bmc
e
u
U
f e 0
t
T
I 1
#ifndef RWLISTITER_H
#define RWLISTITER_H

#include "BTConfig.H"

#include <assert.h>

#include "ListIter.H"

template<class T>
class RWListIter : public ListIter<T> {
friend class List<T>;

private:
  inline void check_iters();
protected:
  inline void update_removal(ListElement<T> *old_elem); 
  inline void update_insertion();

public:
  RWListIter (List<T>& list) : ListIter<T> (list) { list.add_rw_iter_ (this); }
  ~RWListIter();
  inline void insert_before(T& data);
  inline void insert_after (T& data);
  inline int remove_prev (T& data);
  inline int remove_next (T& data);
};

template<class T> RWListIter<T>::~RWListIter() {
  ((List<T> *) list_)->remove_iter_(this); 
  ((List<T> *) list_)->remove_rw_iter_(this); 
}

template<class T> void RWListIter<T>::update_removal (ListElement<T> *old_elem) {
  AbsListIter::update_removal (old_elem);
} 

template<class T> void RWListIter<T>::update_insertion () {
  AbsListIter::update_insertion();
} 

template<class T> void RWListIter<T>::check_iters() {
  if (((List<T> *) list_)->iters_() > 1) {
    // A write operation is being attempted, but there
    // are still read-only iters on the list 
    assert (1 == 0);
  }
}  

template<class T> void RWListIter<T>::insert_before (T& data) {
  check_iters();
  ListElement<T> *new_elem;
  new_elem = new ListElement<T> (data);
  AbsListIter::insert_before (new_elem);
}  

template<class T> void RWListIter<T>::insert_after (T& data) {
  check_iters();
  ListElement<T> *new_elem;
  new_elem = new ListElement<T> (data);
  AbsListIter::insert_after (new_elem);
}  

template<class T> int RWListIter<T>::remove_prev (T& data) {
  check_iters();
  ListElement<T> *old_elem = (ListElement<T> *) AbsListIter::remove_prev(); 
  if (!old_elem)
    return 0;
  data = old_elem->data();
  delete old_elem;
  return (1);
}    

template<class T> int RWListIter<T>::remove_next (T& data) {
  check_iters();
  ListElement<T> *old_elem = (ListElement<T> *) AbsListIter::remove_next(); 
  if (!old_elem)
    return 0;
  data = old_elem->data();
  delete old_elem;
  return (1);
}    

#endif 
E 1
