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
