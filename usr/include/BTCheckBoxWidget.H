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
    
		toggle_struct_ = 0;
    
		XtAddCallback(me_, XmNvalueChangedCallback, toggle_CB,
		    (XtPointer)this);
	}

	~BTCheckBoxWidget()
	{
		if (toggle_struct_)
			delete toggle_struct_;
	}

	static void
	toggle_CB(Widget widget, XtPointer data, XtPointer call_data)
	{
		BTCheckBoxWidget *t = (BTCheckBoxWidget *)data;
		XmToggleButtonCallbackStruct *state =
		    (XmToggleButtonCallbackStruct *)call_data;
		t->state_ = state->set;
		t->callback(t->toggle_struct_);
	}

	virtual void
	addToggleCallback(void (*cb)(BTWidget *, void*), void *data)
	{
		addCallback( toggle_struct_, cb, data );
	}

	int state_;

protected:
	ActivateCBStruct *toggle_struct_;
};

#endif
