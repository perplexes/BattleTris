h51424
s 00035/00028/00030
d D 1.2 01/10/21 19:25:13 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:18 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTCheckBoxWidget.H
c Name history : 1 0 src/widget/BTCheckBoxWidget.H
e
s 00058/00000/00000
d D 1.1 01/10/20 13:35:17 bmc 1 0
c date and time created 01/10/20 13:35:17 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTCheckBoxWidget.H                                  */
/*    DATE: Tue Apr  5 02:55:13 1994                            */
/****************************************************************/

#ifndef BTCHECKBOXWIDGET_H
#define BTCHECKBOXWIDGET_H

#include "BTConfig.H"

#include <Xm/RowColumn.h>
#include <Xm/ToggleBG.h>

#include "BTWidget.H"

class BTCheckBoxWidget : public BTWidget {
public:
D 3
  BTCheckBoxWidget(BTWidget *parent, char *const name, char *const label,
                   int defval = 0)
  : BTWidget(parent) {
    XmString str = XmStringCreateSimple(label);
    me_ = XtVaCreateWidget(name, xmToggleButtonGadgetClass,
			   parent ? parent->getWidget() : 0,
			   XmNset, defval ? True : False, XmNlabelString, str,
			   XmNindicatorType, XmN_OF_MANY, NULL);
    XmStringFree(str);
E 3
I 3
	BTCheckBoxWidget(BTWidget *parent, const char *name,
	    const char *label, int defval = 0)
	: BTWidget(parent)
	{
		XmString str = XmStringCreateSimple((char *)label);
		me_ = XtVaCreateWidget(name, xmToggleButtonGadgetClass,
		    parent ? parent->getWidget() : 0,
		    XmNset, defval ? True : False, XmNlabelString, str,
		    XmNindicatorType, XmN_OF_MANY, NULL);
		    XmStringFree(str);
E 3
    
D 3
    toggle_struct_ = 0;
E 3
I 3
		toggle_struct_ = 0;
E 3
    
D 3
    XtAddCallback(me_, XmNvalueChangedCallback, toggle_CB, (XtPointer) this);
E 3
I 3
		XtAddCallback(me_, XmNvalueChangedCallback, toggle_CB,
		    (XtPointer)this);
	}
E 3

D 3
  }
E 3
I 3
	~BTCheckBoxWidget()
	{
		if (toggle_struct_)
			delete toggle_struct_;
	}
E 3

D 3
  ~BTCheckBoxWidget() {
  if ( toggle_struct_ )
     delete toggle_struct_;
  }
E 3
I 3
	static void
	toggle_CB(Widget widget, XtPointer data, XtPointer call_data)
	{
		BTCheckBoxWidget *t = (BTCheckBoxWidget *)data;
		XmToggleButtonCallbackStruct *state =
		    (XmToggleButtonCallbackStruct *)call_data;
		t->state_ = state->set;
		t->callback(t->toggle_struct_);
	}
E 3

D 3
  static void toggle_CB(Widget widget, XtPointer data, XtPointer call_data) {
    BTCheckBoxWidget *t = (BTCheckBoxWidget *)data;
    XmToggleButtonCallbackStruct *state =
      (XmToggleButtonCallbackStruct *) call_data;
    t->state_ = state->set;
    t->callback( t->toggle_struct_ );
  }
  virtual void addToggleCallback( void (*cb)(BTWidget *, void*), void *data ) {
    addCallback( toggle_struct_, cb, data );
  }
E 3
I 3
	virtual void
	addToggleCallback(void (*cb)(BTWidget *, void*), void *data)
	{
		addCallback( toggle_struct_, cb, data );
	}
E 3

D 3
  int state_;
E 3
I 3
	int state_;
E 3

protected:
D 3
  ActivateCBStruct *toggle_struct_;
E 3
I 3
	ActivateCBStruct *toggle_struct_;
E 3
};

#endif
E 1
