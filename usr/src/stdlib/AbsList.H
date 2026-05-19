#ifndef ABSLIST_H
#define ABSLIST_H

#include "BTConfig.H"
#include "AbsListElement.H"

//  class AbsList
//
//  Contains most of the engine of the actual list.  Most methods
//  are self-explanatory, with the possible exception of update_*
//  These methods are called by the list iter when it has deleted
//  an element, so the list can keep its head and tail pointers 
//  pointing to the appropriate elements.

class AbsList {
friend class AbsListIter;

protected:
  AbsListElement *head_, *tail_;  
  AbsList() : head_ (0), tail_ (0) {}
  AbsListElement *head() { return head_; }
  AbsListElement *tail() { return tail_; }
  void insert_before_head (AbsListElement *new_elem);
  void insert_after_tail (AbsListElement *new_elem);
  AbsListElement *remove_head();
  AbsListElement *remove_tail();
  int empty();
  void update_removal (AbsListElement *old_elem);
  void update_insertion ();
};

#endif
