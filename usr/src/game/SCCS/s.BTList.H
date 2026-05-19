h36809
s 00000/00000/00000
d R 1.2 01/10/20 13:35:39 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTList.H
c Name history : 1 0 src/game/BTList.H
e
s 00043/00000/00000
d D 1.1 01/10/20 13:35:38 bmc 1 0
c date and time created 01/10/20 13:35:38 by bmc
e
u
U
f e 0
t
T
I 1
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
E 1
