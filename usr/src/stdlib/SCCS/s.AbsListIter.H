h26336
s 00000/00000/00000
d R 1.2 01/10/20 13:35:03 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/stdlib/AbsListIter.H
c Name history : 1 0 src/stdlib/AbsListIter.H
e
s 00030/00000/00000
d D 1.1 01/10/20 13:35:02 bmc 1 0
c date and time created 01/10/20 13:35:02 by bmc
e
u
U
f e 0
t
T
I 1
#ifndef ABSLISTITER_H
#define ABSLISTITER_H

#include "BTConfig.H"
#include "AbsList.H"
#include "AbsListElement.H"

class AbsListIter {
protected:
  AbsList *list_;
  AbsListElement *next_, *prev_;
  AbsListIter (AbsList *list) 
  : list_ (list), next_ (list->head()), prev_ (0) {}
  void inc();
  void dec();
  AbsListElement *peek_next() { return next_; }
  AbsListElement *peek_prev() { return prev_; }
  void update_removal (AbsListElement *old_elem);
  void update_insertion();
  AbsListElement *remove_next();
  AbsListElement *remove_prev();
  void insert_after (AbsListElement *new_elem);
  void insert_before (AbsListElement *new_elem);
  int remove_next(AbsListElement& data);
  int remove_prev(AbsListElement& data);
  void jump_before_head();
  void jump_after_tail();
};
  
#endif
E 1
