h26166
s 00000/00000/00000
d R 1.2 01/10/20 13:35:20 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTXDisplay.H
c Name history : 1 0 src/widget/BTXDisplay.H
e
s 00036/00000/00000
d D 1.1 01/10/20 13:35:19 bmc 1 0
c date and time created 01/10/20 13:35:19 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: BTXDisplay.H                                        */
/*    ASGN:                                                     */
/*    DATE: Thu Sep 28 11:55:57 1995                            */
/****************************************************************/


#ifndef BTXDISPLAY_HEADER
#define BTXDISPLAY_HEADER

#include <Xm/Xm.h>
#include "BTDisplay.H"
#include "Block.H"

class BTXDisplay : public BTDisplay {

public:
  BTXDisplay();
  ~BTXDisplay();
  virtual void newPalette();
  virtual void handleEvents();
  virtual unsigned long addTimeout( unsigned long, void (*)(void *, unsigned long *),
			    void * );
  virtual void removeTimeout( unsigned long );
//  virtual void flush() ( XFlush(display_) );
  void addHandler( Widget );
  void flush() { XFlush(display_); }
  XtAppContext app_;

  Widget toplevel_;
  Display *display_;
};

#endif
E 1
