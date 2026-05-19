h15428
s 00000/00000/00000
d R 1.2 01/10/20 13:35:06 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/stdlib/AbsList.C
c Name history : 1 0 src/stdlib/AbsList.C
e
s 00078/00000/00000
d D 1.1 01/10/20 13:35:05 bmc 1 0
c date and time created 01/10/20 13:35:05 by bmc
e
u
U
f e 0
t
T
I 1
#include "BTConfig.H"
#include "AbsList.H"

void AbsList::insert_before_head (AbsListElement *new_elem) {
  new_elem->prev(0); 
  new_elem->next(head_);
  if (head_)
    head_->prev (new_elem);
  else
    tail_ = new_elem;
  head_ = new_elem;
}

void AbsList::insert_after_tail (AbsListElement *new_elem) {
  new_elem->next(0); 
   new_elem->prev(tail_);
  new_elem->prev(tail_);
  if (tail_) 
    tail_->next(new_elem);
  else
    head_ = new_elem;
  tail_ = new_elem;
}

AbsListElement *AbsList::remove_head() {
  AbsListElement *old_head = head_;
  head_ = old_head->next();
  if (head_) 
    head_->prev(0);
  else
    tail_ = 0;
  return old_head;
}

AbsListElement *AbsList::remove_tail() {
  AbsListElement *old_tail = tail_;
  tail_ = old_tail->prev();
  if (tail_) 
    tail_->next(0);
  else
    head_ = 0;
  return old_tail;
}

int AbsList::empty() {
  return head_ == (AbsListElement *) 0;
}

//
//  update_removal
//
//  We don't want to get into a situation where an item has been
//  removed from the list by an iter, and our head or tail now
//  points to invalid data.
//

void AbsList::update_removal (AbsListElement *old_elem) {
  if (old_elem == head_) 
    head_ = head_->next();
  if (old_elem == tail_)
    tail_ = tail_->prev();
  old_elem->next(0);
  old_elem->prev(0);
}

//
//  update_insertion
//
//  Same shit, different method.
//

void AbsList::update_insertion() {
  if (!head_ || !tail_) return;
  if (head_->prev())
    head_ = head_->prev();
  if (tail_->next())
    tail_ = tail_->next();
}
E 1
