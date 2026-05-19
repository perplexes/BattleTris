/****************************************************************/
/*    NAME: Charles G. Hoecker                                  */
/*    ACCT: cgh                                                 */
/*    FILE: Btstack.H                                        */
/*    ASGN: Final                                               */
/*    DATE: Mon Oct  3 15:54:18 1994                            */
/****************************************************************/

#ifndef BTSTACK_H
#define BTSTACK_H

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#define STACK_SIZE 50

template<class T>
class BTStack {
public:
  BTStack() : mem_no_(STACK_SIZE), mem_(NULL),
    mem_tail_(NULL), mem_end_(NULL), mem_ptr_(NULL),
    count_(0) {};
  ~BTStack() { free(mem_); };
  void operator=(BTStack<T> &);
  void tailInsert(const T&);
  int tailRemove(T&);
  void clear();
  inline void headJump() { mem_ptr_ = mem_; };
  inline void tailJump() { mem_ptr_ = mem_tail_; };
  int getNext(T&);
  int getPrev(T&);
  inline void inc() { if (mem_ptr_ < mem_tail_) mem_ptr_ += sizeof(T); }
  inline void dec() { if (mem_ptr_ > mem_) mem_ptr_ -= sizeof(T); }

private:
  char *mem_tail_,*mem_,*mem_end_,*mem_ptr_;
  unsigned long mem_no_;
  unsigned long count_;
};


#endif
