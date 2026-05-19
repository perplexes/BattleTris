/****************************************************************/
/*    NAME: Charles G. Hoecker                                  */
/*    ACCT: cgh                                                 */
/*    FILE: BTList.H                                        */
/*    ASGN: Final                                               */
/*    DATE: Mon Oct  3 15:54:18 1994                            */
/****************************************************************/

#ifndef BTLIST_H
#define BTLIST_H

#include "BTConfig.H"

#if STDC_HEADERS
# include <stdlib.h>
#endif

#define BT_MOVE_MEM_NO 50

template<class T>
class BTList {
public:
  BTList() : move_mem_no_(BT_MOVE_MEM_NO), move_mem_(NULL),
    move_mem_tail_(NULL), move_mem_end_(NULL), move_mem_ptr_(NULL),
    count_(0) {};
  ~BTList() { free(move_mem_); };
  void operator=(BTList<T> &);
  T *tailInsert( int, int, BT_MOVE_DIR );
  inline void headJump() { move_mem_ptr_ = move_mem_; };
  inline void tailJump() { move_mem_ptr_ = move_mem_tail_; };
  T *tailRemove();
  void clear();
  T *getNext();
  T *getPrev();

private:
  char *move_mem_tail_,*move_mem_,*move_mem_end_,*move_mem_ptr_;
  int move_mem_no_;
  int count_;
};


#endif
