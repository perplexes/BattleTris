#ifndef ABSLISTELEMENT_H
#define ABSLISTELEMENT_H

#include "BTConfig.H"

class AbsListElement {
friend class AbsList;
friend class AbsListIter;

protected:
  AbsListElement *prev_, *next_;
  AbsListElement(AbsListElement *prev, AbsListElement *next) 
  : prev_ (prev), next_ (next) {};
  AbsListElement *prev() { return prev_; }
  AbsListElement *next() { return next_; }
  void prev (AbsListElement *prev) { prev_ = prev; }
  void next (AbsListElement *next) { next_ = next; }
};

#endif
