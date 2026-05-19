/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTScrolledListWidget.C                              */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 13:18:03 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTScrolledListWidget.H"

BTScrolledListWidget::BTScrolledListWidget(BTWidget *parent, const char *name,
    const char **list, int n)
: BTWidget(parent), string_(0)
{
	XmStringTable strtab;
	Arg args[10];
	int i = 0, s;

	XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
	XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
	XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
	XtSetArg(args[i], XmNlistSizePolicy, XmCONSTANT); i++;
	XtSetArg(args[i], XmNselectionPolicy, XmBROWSE_SELECT); i++;
	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNscrollVertical, True); i++;

	if (list) {
		strtab = (XmStringTable) XtMalloc(n * sizeof(XmString));

		for (s = 0; s < n; s++)
			strtab[s] = XmStringCreateSimple((char *)list[s]);

		XtSetArg(args[i], XmNvisibleItemCount, n); i++;
		XtSetArg(args[i], XmNitemCount, n); i++;
		XtSetArg(args[i], XmNitems, strtab); i++;
  	}

	list_w_ = XmCreateScrolledList(parent ? parent->getWidget() : 0,
	    (char *)name, args, i);

	if (list) {
		for (s = 0; s < n; s++)
			XmStringFree(strtab[s]);
		XtFree((caddr_t) strtab);
	}

	def_action_ = browse_sel_ = 0;
	XtAddCallback(list_w_, XmNdefaultActionCallback, defAction_CB, this);
	XtAddCallback(list_w_, XmNbrowseSelectionCallback, browseSel_CB, this);

	me_ = XtParent(list_w_); // Get the scrolled window widget

	XtManageChild(list_w_);
}

BTScrolledListWidget::BTScrolledListWidget(BTWidget *parent, const char *name,
   const char **list, int n, Position x1, Position y1, Position x2, Position y2)
: BTWidget(parent), string_(0)
{
	XmStringTable strtab;
	Arg args[18];
	int i = 0, s;

	XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
	XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
	XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
	XtSetArg(args[i], XmNlistSizePolicy, XmCONSTANT); i++;
	XtSetArg(args[i], XmNselectionPolicy, XmBROWSE_SELECT); i++;
	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNscrollVertical, True); i++;


	XtSetArg(args[i], XmNtopAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNtopPosition, y1); i++;
	XtSetArg(args[i], XmNleftAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNleftPosition, x1); i++;
	XtSetArg(args[i], XmNrightAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNrightPosition, x2); i++;
	XtSetArg(args[i], XmNbottomAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNbottomPosition, y2); i++;

	if (list) {
		strtab = (XmStringTable)XtMalloc(n * sizeof(XmString));

		for (s = 0; s < n; s++)
			strtab[s] = XmStringCreateSimple((char *)list[s]);

		XtSetArg(args[i], XmNvisibleItemCount, n); i++;
		XtSetArg(args[i], XmNitemCount, n); i++;
		XtSetArg(args[i], XmNitems, strtab); i++;
	}

	list_w_ = XmCreateScrolledList(parent ? parent->getWidget() : 0,
	    (char *)name, args, i);

	if (list) {
		for (s = 0; s < n; s++)
			XmStringFree(strtab[s]);
		XtFree((caddr_t) strtab);
	}

	def_action_ = browse_sel_ = 0;
	XtAddCallback(list_w_, XmNdefaultActionCallback, defAction_CB, this);
	XtAddCallback(list_w_, XmNbrowseSelectionCallback, browseSel_CB, this);

	me_ = XtParent(list_w_); // Get the scrolled window widget
	XtManageChild(list_w_);
}

void
BTScrolledListWidget::setList(const char **list, int n)
{
	XmStringTable strtab;
	int i;

	XmListDeselectAllItems(list_w_);
	XmListDeleteAllItems(list_w_);

	if (list == NULL || n == 0)
		return;

	strtab = (XmStringTable)XtMalloc(n * sizeof(XmString));

	for(i = 0; i < n; i++)
		strtab[i] = XmStringCreateSimple((char *)list[i]);

	XtVaSetValues(list_w_,
	    XmNvisibleItemCount, n,
	    XmNitemCount, n,
	    XmNitems, strtab,
	    NULL);

	XmListSelectItem(list_w_, strtab[0], False);

	for (i = 0; i < n; i++)
		XmStringFree(strtab[i]);
	XtFree((caddr_t)strtab);
}
