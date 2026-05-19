h51069
s 00000/00000/00000
d R 1.2 01/10/20 13:35:20 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTXDisplay.C
c Name history : 1 0 src/widget/BTXDisplay.C
e
s 00053/00000/00000
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
/*    FILE: BTXDisplay.C                                        */
/*    ASGN:                                                     */
/*    DATE: Thu Sep 28 11:56:34 1995                            */
/****************************************************************/

#include "BTXDisplay.H"
#include "BTConstants.H"
#include <iostream.h>
#include "BTXPalette.H"

extern Widget g_toplevel;
extern XtAppContext g_appctx;
extern Colormap g_colormap;

BTXDisplay::BTXDisplay() : toplevel_(g_toplevel), app_(g_appctx) {
  display_ = XtDisplay(g_toplevel);
  palette_ = new BTXPalette(g_colormap);
}

BTXDisplay::~BTXDisplay() {
}

void BTXDisplay::addHandler( Widget w ) {
}

void BTXDisplay::handleEvents() {
  
  XtInputMask mask;
  
  while ( (mask = XtAppPending ( app_ )) ) {
    XtAppProcessEvent( app_, mask );
  }

}

unsigned long BTXDisplay::addTimeout( unsigned long delay,
			    void (*func)(void *, unsigned long *),
			    void *data )
{
  return XtAppAddTimeOut( app_, delay, func, data );
}

void BTXDisplay::removeTimeout( unsigned long id ) {
  XtRemoveTimeOut( id );
}

void BTXDisplay::newPalette() {
  palette_->createNew();
}

E 1
