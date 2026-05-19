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
