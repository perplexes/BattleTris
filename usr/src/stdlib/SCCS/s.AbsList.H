h35470
s 00000/00000/00000
d R 1.2 01/10/20 13:35:05 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/stdlib/AbsList.H
c Name history : 1 0 src/stdlib/AbsList.H
e
s 00032/00000/00000
d D 1.1 01/10/20 13:35:04 bmc 1 0
c date and time created 01/10/20 13:35:04 by bmc
e
u
U
f e 0
t
T
I 1
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
E 1
