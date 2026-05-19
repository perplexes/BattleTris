/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTScrolledList.H                                    */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 13:15:15 1994                            */
/****************************************************************/

#ifndef BTSCROLLEDLIST_H
#define BTSCROLLEDLIST_H

#include "BTConfig.H"

#include <Xm/List.h>

#include "BTWidget.H"

class BTScrolledListWidget : public BTWidget {
protected:
	Widget list_w_;

public:
	BTScrolledListWidget(BTWidget *parent, const char *name,
	    const char **list, int n);
	BTScrolledListWidget(BTWidget *parent, const char *name,
	    const char **list, int n,
	    Position x1, Position y1, Position x2, Position y2);

	~BTScrolledListWidget()
	{
		XtDestroyWidget(list_w_);
		if (def_action_)
			delete def_action_;

		if (browse_sel_)
			delete browse_sel_;

		if (string_)
			XtFree(string_);
		me_ = NULL;
	}

	Widget getListWidget() { return (list_w_); }
	void setList(const char **list, int n);

	void
	selectPos(int pos, Boolean notify = 0)
	{
		XmListSelectPos( list_w_, pos, notify );
	}

	static void
	defAction_CB(Widget widget, XtPointer data, XtPointer call_data)
	{
		BTScrolledListWidget *t = (BTScrolledListWidget *)data;
		XmListCallbackStruct *cbs = (XmListCallbackStruct *)call_data;
		t->last_selection_ = cbs->item_position - 1;
		if (t->string_)
			XtFree(t->string_);
		XmStringGetLtoR(cbs->item, XmFONTLIST_DEFAULT_TAG, &t->string_);
		t->callback(t->def_action_);
	}

	virtual void
	addDefActionCallback(void (*cb)(BTWidget*, void*), void *data)
	{
		addCallback(def_action_, cb, data);
	}

	static void
	browseSel_CB(Widget widget, XtPointer data, XtPointer call_data)
	{
		BTScrolledListWidget *t = (BTScrolledListWidget *)data;
		XmListCallbackStruct *cbs = (XmListCallbackStruct *)call_data;
		t->last_selection_ = cbs->item_position - 1;
		if (t->string_)
			XtFree(t->string_);

		XmStringGetLtoR(cbs->item, XmFONTLIST_DEFAULT_TAG, &t->string_);
		t->callback(t->browse_sel_);
	}

	virtual void
	addBrowseSelCallback(void (*cb)(BTWidget*, void*), void *data)
	{
		addCallback(browse_sel_, cb, data);
	}

	ActivateCBStruct *def_action_, *browse_sel_;
	int last_selection_;
	char *string_;
};

#endif
