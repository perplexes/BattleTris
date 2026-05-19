#include "BTConfig.H"
#include "AbsListIter.H"

void AbsListIter::inc() {
  if (!next_) 
    return;
  prev_ = next_;
  next_ = next_->next();
}

void AbsListIter::dec() {
  if (!prev_) return;
  next_ = prev_;
  prev_ = prev_->prev();
}  

void AbsListIter::update_insertion() {

  // This is kind of weird behavior:  when we update our insertion,
  // we don't change our prev pointer...instead we change our 
  // next pointer

  if (!next_) return;
  if (next_->prev() != prev_) 
    next_ = next_->prev();
}

void AbsListIter::update_removal (AbsListElement *old_elem) {
  if (prev_ == old_elem)
    prev_ = old_elem->prev();
  if (next_ == old_elem)
    next_ = old_elem->next();
}

AbsListElement *AbsListIter::remove_prev() {
  if (!prev_) return 0;
  AbsListElement *old_elem = prev_;
  prev_ = prev_->prev();
  if (prev_)
    prev_->next (next_);
  if (next_)
    next_->prev (prev_);
  list_->update_removal (old_elem);
  return old_elem;
}

AbsListElement *AbsListIter::remove_next() {
  if (!next_) return 0;
  AbsListElement *old_elem = next_;
  next_ = next_->next();
  if (prev_) 
    prev_->next (next_);
  if (next_)
    next_->prev (prev_);
  list_->update_removal (old_elem);
  return old_elem;
}       
  
void AbsListIter::insert_before (AbsListElement *new_elem) {
  new_elem->prev(prev_);
  new_elem->next(next_);
  if (next_)
    next_->prev (new_elem);
  if (prev_)
    prev_->next (new_elem);
  prev_ = new_elem;
  list_->update_insertion();
}

void AbsListIter::insert_after (AbsListElement *new_elem) {
  new_elem->prev(prev_);
  new_elem->next(next_);
  if (next_)
    next_->prev (new_elem);
  if (prev_)
    prev_->next (new_elem);
  next_ = new_elem;
  list_->update_insertion();
}

void AbsListIter::jump_before_head() {
  next_ = list_->head();
  prev_ = 0;
}

void AbsListIter::jump_after_tail() {
  next_ = 0;
  prev_ = list_->tail();
}
