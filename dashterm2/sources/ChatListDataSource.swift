//
//  ChatListDataSource.swift
//  DashTerm2
//
//  Created by George Nachman on 2/11/25.
//

import AppKit

protocol ChatListDataSource: AnyObject, ChatSearchResultsDataSource {
    func numberOfChats(in chatListViewController: ChatListViewController) -> Int
    // BUG-f611: Changed return type to optional to safely handle out of bounds index
    func chatListViewController(_ chatListViewController: ChatListViewController, chatAt index: Int) -> Chat?
    func chatListViewController(_ viewController: ChatListViewController, indexOfChatID: String) -> Int?
    func snippet(forChatID: String) -> String?
    func firstIndex(forGuid guid: String) -> Int?
}
